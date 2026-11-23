# Syscall implementation handoff

## Repository state

- Branch: `syscall`.
- Implementation commit:
  `ce5fe23512ce0475c78659ae9cf913fae5a5ba8d`.
- Commit subject:
  `added EL1 LED syscalls with const-generic SVC numbers`.
- Parent: `244bafdc`, on the original `24-run-init` branch.
- Nothing was pushed.
- This handoff was generated after the implementation commit
  and committed separately at the user's request. The
  implementation worktree was clean before creating this file.

## Accepted scope and decisions

The user initially asked about a user/kernel LED syscall and
EL0 execution. They then explicitly reduced the scope: keep
`init` at EL1 and add the syscall mechanism and LED operation.
They also requested disabling the existing QEMU init execution
fixture, which depended on a direct kernel function callback.

The final ABI encodes syscall numbers in SVC's immediate. An
earlier intermediate implementation used x8; that design was
superseded. Do not restore register-based syscall selection.

The wrapper remains `#[inline]`. The user asked whether
`#[inline(always)]` was necessary; the explanation was that
const-generic specialization resolves the assembly constant
independently of inlining. An out-of-line wrapper can execute
SVC, resume after it via ERET, and return normally. No attribute
change was requested or made after that discussion.

EL0 entry, separate syscall stacks, scheduling, process exit,
and additional syscalls are deferred. They are not unfinished
requirements of the implemented EL1 milestone.

## Current ABI

The shared `flamingos-syscall` crate is `no_std` and lives in
`libraries/syscall`. The kernel and standalone init depend on it.
Init remains excluded from the root Cargo workspace and uses
its own Cargo configuration and lockfile.

```rust
pub fn call<const NUMBER: u16>(argument: u64) -> i64;
pub fn set_led(on: bool) -> i64;
```

- `NUMBER` is substituted into `svc #{number}` through an
  inline-assembly `const` operand.
- `u16` matches the instruction's immediate field.
- x0 carries the argument and signed result.
- The wrapper does not pass a syscall number in x8.
- Other general registers and saved processor status are
  restored by the existing exception assembly.
- The wrapper keeps the default assembly memory effects.
  Do not add `nomem`: the handler writes device registers.
- The ABI accepts scalar values only; it has no user pointers.

| Number | Operation | Argument |
|---|---|---|
| 1 | `SYS_SET_LED` | Exactly 0 = off, 1 = on |

| Result | Meaning |
|---|---|
| 0 | Success |
| -22 | Invalid LED argument |
| -19 | No supported initialized LED backend |
| -38 | Unknown syscall number |

`set_led(on)` calls `call::<SYS_SET_LED>(u64::from(on))`.
The generated init executable was inspected and contains
`svc #1` for LED transitions.

## Execution and dispatch

Init still starts at EL1h, with SPSR `0x3c5`, asynchronous
exceptions masked, and a guarded 256 KiB process stack.
SVC takes an EL1-to-EL1 synchronous exception using that same
stack. This is not yet a privilege or address-space switch.
TTBR0/TTBR1 and page permission policies are unchanged.

The existing assembly saves x0 through x30, ELR_EL1, SPSR_EL1,
and ESR_EL1 in a 272-byte frame. The Rust current-EL handler:

1. Recognizes `SVC64` from ESR_EL1.
2. Extracts the immediate from the syndrome's low 16 bits.
3. Dispatches that number and saved x0.
4. Stores the signed result into saved x0 as its u64 bits.
5. Returns to the assembly restore path and ERET.

ELR_EL1 already points after SVC and is not advanced manually.
Non-SVC synchronous exceptions still use the diagnostic panic
path. Lower-AArch64 exception handlers still panic; they are
not connected to this dispatcher yet.

There is no special test-only bypass for `svc #0x1337` anymore.
It now goes through normal dispatch and returns -38.

## LED ownership and init behavior

The BSP creates a private LED target during GPIO post-init,
using recognized board wiring and a validated kernel MMIO alias.
`set_activity_led(bool)` owns runtime LED access through
`IRQSafeNullLock<Option<ActivityLed>>`. Its critical section
masks IRQs. Other cores remain parked; this lock is not an SMP
mutex. The existing raw LED helper performs volatile function
selection and SET/CLR writes behind this service.

Supported wiring remains:

| Board | Pin | Polarity |
|---|---|---|
| Zero 2 W | GPIO29 | Active-low |
| Pi 3 A+ / B+ | GPIO29 | Active-high |
| Pi 4 Model B | GPIO42 | Active-high |

Pi 3 Model B without the plus uses unsupported virtual ACT GPIO.
An unsupported or unrecognized board leaves the backend absent.
Init now launches regardless of LED availability; its first
failed LED request reaches its panic handler, which spins.

`ActiveProcess::run()` takes no GPIO or callback arguments.
All initial general registers remain zero. Init's `_start()` is
argument-free and uses the shared wrapper for on/off requests.
Its approximately 250 ms delays still read the physical counter
directly at EL1. The kernel boot diagnostic uses the same kernel
LED service; the pre-MMU boot tracer retains physical access.

## Disabled fixture and active tests

The legacy init execution fixture is explicitly disabled using
`#[cfg(any())]` on `InitArguments`, `test_run_init`, and its
completion callback in `kernel/src/process.rs`. The source is
retained for reference and still describes the obsolete ABI.
Simply removing the cfg guards will not restore a working test.

QEMU boot runs process preparation, stack, activation, alias,
and teardown checks, prints the fixture-disabled message, then
exits successfully without entering the loaded init executable.

Automatic fixture preparation was removed from Make integration
tests, xtask test builds, and the kernel build script's
environment.
The legacy `cargo xtask test-init` command and init's `qemu_test`
feature remain, but do not run a completion test.

`kernel/tests/03_exception_restore_sanity.rs` still exercises an
SVC from a nested function, now through `call::<0x1337>(0)`, and
asserts -38. It also calls BSP `test_led_syscalls()`, which uses
real SVC entry/return against kernel-owned GPIO-shaped RAM.
Coverage includes:

- Repeated successful on/off calls.
- Active-low GPIO29 and active-high GPIO42, covering both banks.
- Function-select neighboring bits and other register contents.
- Invalid arguments 2 and `u64::MAX` returning -22.
- Unknown immediates 0, `0xffff`, and `0x8001` returning -38.
- Fresh register sentinels proving errors do not write GPIO.
- Missing hardware returning -19.
- Preservation of DAIF masks and continued EL1 execution.

These tests do not prove loaded-init execution or EL0 isolation.

## Validation already completed

After the final change to SVC immediates, the following passed:

```sh
make test_integration BSP=rpi3 \
  TEST=03_exception_restore_sanity
make clippy BSP=rpi3
make clippy BSP=rpi4
cargo clippy -p flamingos-syscall \
  --target aarch64-unknown-none-softfloat -- \
  -D warnings -D missing_docs
cargo xtask build rpi4
cargo xtask build rpi3
```

From the `init` directory, this also passed:

```sh
cargo clippy --release --features bsp_rpi3 -- -D warnings
```

ELF inspection reported entry `0x210260`, two loadable segments,
and two prepared pages. Disassembly confirmed the `svc #1`
encoding. Formatting and `git diff --check` passed.

Earlier, before replacing the x8 ABI with SVC immediates,
`make test BSP=rpi3` passed QEMU boot and all five integration
tests. Only the focused exception/syscall integration test was
rerun after the immediate change; the full suite was not rerun.
Tests were not repeated merely to create the commit.

Both normal BSP images build, but Pi 4 has no configured QEMU
machine in this Makefile. Physical blinking through the new
syscall path has not been verified. Historical direct-init
blinking worked on Pi 3 A+; the Zero 2 W physical ACT issue
described in README remains unresolved.

## Main implementation locations

- `libraries/syscall/src/lib.rs`: constants and client wrapper.
- `kernel/src/syscall.rs`: scalar validation and dispatch.
- `kernel/src/_arch/aarch64/exception.rs`: syndrome decoding.
- `kernel/src/_arch/aarch64/exception.s`: existing save/restore.
- `kernel/src/bsp/raspberrypi/driver.rs`: backend and GPIO tests.
- `kernel/src/_arch/aarch64/cpu/led.rs`: raw LED helper.
- `kernel/src/process.rs`: launch ownership and disabled fixture.
- `kernel/src/_arch/aarch64/process.rs`: initial EL1 context.
- `kernel/src/main.rs`: init launch and QEMU boot completion.
- `init/src/main.rs`: syscall-driven blink loop.
- `kernel/tests/03_exception_restore_sanity.rs`: SVC integration.
- `README.md`: behavior, build instructions, and limitations.

Normal builds embed the matching init in a sparse filesystem.
Use `cargo xtask build rpi3`, `rpiz2`, or `rpi4` as appropriate.
SD-card boot requires refreshing init.elf as well as the kernel:
the old direct-GPIO init ABI is no longer supplied at launch.
The last generated root `kernel8.img` was the Pi 3 build from
before this commit and still carries the earlier dirty revision.
Rebuild before hardware testing if the embedded revision matters.

## Follow-up boundaries

The next useful acceptance step is a hardware blink check with
matching kernel and init artifacts. Restoring an init execution
fixture should use a deliberately designed completion mechanism
instead of reviving the obsolete kernel callback ABI.

If EL0 work is later authorized, it requires process-specific
EL0 page permissions, UXN/PXN handling, SP_EL0 setup while
preserving a valid SP_EL1, lower-AArch64 SVC dispatch, an EL0
counter-access policy, and isolation tests. There is currently
no scheduler, process reclamation, or user-fault recovery policy.

The user supplied repository guidelines in conversation; no
tracked AGENTS.md was found. Keep these constraints:

- Never commit or push without an explicit user request.
- Commit subjects start with a lowercase past-tense verb.
- Agent commits include `Generated-With: Codex` attribution.
- Do not add unsafe code without explicit approval. The user
  approved the small SVC assembly wrapper and the documented
  kernel call to the existing unsafe LED helper for this task.
  This is not blanket approval for future architecture work.
- Unsafe boundaries need Safety and Need documentation.
- Keep Markdown lines near 65 characters where practical.
- Provide concise progress updates while working.

No further implementation task is currently authorized by
this handoff alone.
