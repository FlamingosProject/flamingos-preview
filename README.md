# Tutorial 24 - Run init

This chapter loads `init.elf`, prepares its memory and stack,
activates lower-address translation, and enters it at EL1h.
The process directly drives the activity LED (ACT) GPIO, with
existing kernel and hardware mappings accessible. The implementation is
organized into four steps below; scheduling is still future work.

## Step 1: process memory

`program_image::pages::PreparedImage` consumes the loaded image
and copies its segments into owned, 64 KiB-aligned heap pages.
Each allocation is fallible. Prefix and suffix padding are zeroed;
ELF BSS remains zero. Rounded backing is limited to 8 MiB, and the
null page is reserved. Segments sharing a page are rejected,
including nonoverlapping byte ranges with different permissions.

`process::ProcessMemory::prepare` constructs a private level-2
root and level-3 tables covering the lower 1 GiB. Every backing
page is translated through the existing kernel mappings to find
its physical address; physical contiguity is not assumed.
Segment read/write and execute permissions become privileged page
attributes. Segments without read permission are rejected because
the current page attributes cannot represent them faithfully.

The prepared process owns both its backing and its tables. Their
heap addresses remain stable when their owning Rust values move.
Safe inspection APIs translate addresses and report attributes
without dereferencing the process's virtual addresses. These
tables are not installed by preparation. Failed preparation frees
its allocations without changing live maps. Step 3 transfers their
ownership to an activation guard before installing them.

The kernel's existing TTBR1 mappings remain untouched, including
driver MMIO aliases for GPIO and other hardware. Later privileged
execution can use these same aliases. Physical device addresses
are not automatically identity-mapped in the process's lower
address space. Step 3 enables TTBR0 walks. The stack and initial
context are prepared by Step 2 below.

Boot prepares the image and reports its entry, page count, and
physical root address with `(not active)`. The QEMU boot test
constructs a synthetic image and verifies descriptor translation,
page offsets, permissions, and unmapped addresses. Host tests
cover backing alignment, cross-page copying, zero padding, limits,
and invalid/shared-page rejection. No new unsafe code is needed
to prepare or inspect the inactive mappings.

## Step 2: stack and initial CPU context

`process::Process::prepare` owns a `ProcessMemory` together with
an AArch64 `InitialContext`. The lower address space reserves its
top four pages for a descending, zeroed 256 KiB stack. Its backed
range is `[0x3ffc0000, 0x40000000)`, with an unmapped guard at
`[0x3ffb0000, 0x3ffc0000)`. ELF segments must end before the guard.
Stack pages use cacheable, privileged read/write, execute-never
attributes. Allocations remain fallible, and the guard consumes
no backing memory.

The context starts with the ELF entry as PC, SP at `0x40000000`,
and all x0–x30 registers zero. SP is the exclusive end of the
stack, aligned to 16 bytes as required by AArch64; stack writes
use addresses below it. Step 4 supplies scalar boot arguments in
x0–x3; there is no argc, argv, environment, or return trampoline.
The link register is zero, so `init` must never return.

Saved status is `0x3c5`: EL1h uses SP_EL1, with debug, SError,
IRQ, and FIQ masked. This status is metadata only; constructing
the context does not modify the running CPU's registers or
interrupt state. The context has an explicit C layout and is
separate from the existing exception-handler stack frame. Step 4
uses that layout for the final restore and ERET.

Boot prints the prepared PC, SP, and saved status. For the current
`init`, process backing comprises two image pages and four stack
pages, excluding translation tables. QEMU checks physical stack
backing, zero initialization, RW/XN attributes, both ends of the
guard, SP alignment, PC, zeroed registers, and exact saved status.
It also verifies rejection of an ELF segment in the guard range.

## Step 3: activate lower-address translation

`Process::activate` consumes the prepared process and returns an
`ActiveProcess` guard. Activation is restricted to the boot core
at EL1 with the existing kernel MMU/cache configuration and no
other lower root active. It sets TG0 to 64 KiB, T0SZ to 34 for
1 GiB, and cacheable, inner-shareable table-walk attributes.
TTBR1 and its control fields, including ASID selection, remain
unchanged. Device aliases continue to use the kernel's upper map.

Table stores are ordered before use, and whole-regime TLB
invalidation removes stale translations, including global entries.
Lower walks are enabled only after the new root and configuration
are synchronized. Temporary exception masks are restored before
returning to normal kernel execution. This follows the ordering
requirements in [Arm's memory-management guide](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/Learn%20the%20Architecture/LearnTheArchitecture-MemoryManagement-101811_0100_00_en.pdf).

The guard cannot be sent/shared between cores. It owns all process
allocations, and dropping it disables lower walks, flushes cached
aliases, and restores the previous TTBR0/TCR values before freeing
the backing. Only one lower address space is active at a time;
this is bring-up infrastructure, with no scheduler or ASID policy.

`ActiveProcess::translate_hardware` uses AT/PAR to check actual
translations and read/write permissions. Invalid addresses report
translation faults without trapping data accesses. QEMU also
tests actual loads/stores through owned aliases, read-only code,
the unmapped guard, preserved kernel/MMIO mappings, rejection of
a second activation, teardown, and replacement by a different
physical image at the same virtual entry.

The new unsafe architecture boundary has explicit Safety and Need
documentation at its declarations and call sites. Alias tests use
assembly rather than creating duplicate Rust references to the
same storage. These operations were explicitly approved by the
user for Step 3.

Activation itself returns to normal kernel execution. The initial
CPU context remains metadata until the explicit Step 4 launch.

## Step 4: enter init and blink the activity LED

`ActiveProcess::run` supplies GPIO base, LED pin, and polarity in
x0–x2. x3 is zero for normal hardware builds; the QEMU fixture
uses it for a trusted completion callback. Other general registers
remain zero, including LR. The GPIO base is the existing high
device alias, not a physical address or new identity mapping.

Before entering copied code, the kernel cleans its backing via
the high data alias with DC CVAU, orders those writes, invalidates
the lower instruction alias with IC IVAU, then issues DSB/ISB.
Cache-line sizes come from CTR_EL0. The final assembly restores
the prepared SP, PC, status, and general registers, then uses ERET
to enter EL1h. Asynchronous exceptions remain masked, so the
first process runs without timer preemption or UART IRQ handling.

The active owner is retained with `ManuallyDrop`; the kernel
stack, translation tables, and process backing are intentionally
kept alive. There is no process exit or reclamation path yet.
The launch API is unsafe because trusted privileged ELF code can
access kernel memory and hardware. Safety/Need documentation
covers the transfer, cache maintenance, and MMIO boundaries.

Boot debugging, the kernel diagnostic, and init now compile the
same raw `set_led` helper. It uses the debug sequence: select
GPIO output mode, then write SET/CLR. Early boot passes the
physical GPIO base; kernel and init pass its mapped alias.
The helper is available without enabling `boot_trace`.

To enable the pre-MMU boot LED codes (stages 1, 2, and 3):

```sh
cargo xtask build rpiz2 --features boot_trace
```

Upload the resulting `kernel8.img` with the usual `scip` command.
Rebuild without `--features boot_trace` to disable the codes.
These early writes use physical GPIO addresses; blinking here
but not in the kernel diagnostic would implicate the mapped
address path. Zero 2 W uses GPIO29 active-low; the Pi 4 Model B
build selects GPIO42 active-high. Early tracing in the `rpi3`
family assumes Zero 2 W wiring, not Pi 3's virtual ACT GPIO.

Before loading the ELF, the kernel drives ACT on, off, then on
for one second each, logging `Kernel debug ACT`. The final on
state remains until init takes over. Init uses the shared helper
for each transition and polls the generic physical counter for
approximately 250 ms on and 250 ms off, without kernel calls.
The delay busy-waits rather than using timer interrupts.
Firmware board identity selects the verified wiring:

| Board | Activity LED | LED on |
|---|---|---|
| Zero 2 W | GPIO29 | Low |
| Pi 4 Model B | GPIO42 | High |

Sources are the official [Zero 2 W](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm/boot/dts/broadcom/bcm2710-rpi-zero-2-w.dts)
and [Pi 4](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm/boot/dts/broadcom/bcm2711-rpi-4-b.dts)
device trees. Pi 3 Model B uses firmware-managed virtual GPIO;
it does not share Zero 2's direct GPIO29 wiring. Unsupported or
unrecognized boards skip LED launch and retain the kernel console.

The QEMU boot test builds a separate `qemu_test` Rust init ELF,
loads it through the production loader, and performs the actual
context transfer. Init completes an on/off cycle against retained
register-shaped RAM, then calls a kernel completion hook. The
hook checks the process stack range/alignment, EL1 state, masked
exceptions, output mode, and both GPIO writes before exiting QEMU.
This proves executable transfer and MMIO-shaped writes; the
physical LED still needs an on-board acceptance test.

The current Zero 2 W hardware test reaches the init handoff and
reports matching GPIO function/level readbacks, but ACT has not
visibly lit. Pre-MMU tracing and historical LED builds also fail
to light it. The cause remains unresolved; the next checks are
physical LED inspection/testing and another Pi. Treat QEMU's
successful execution as separate from physical LED validation.

`cargo xtask test-init rpi3` builds the test executable under
`target/init-qemu/rpi3` and packages it into a sparse exFAT image
under `target/bootfs-qemu/rpi3`. QEMU mounts that image through
the same read-only adapter and filesystem loader as development
boot. Test builds prepare it automatically, preserving production
artifacts. The test fixture must not be copied to a hardware card.

## Embedded development filesystem

To avoid copying a new `init` onto the SD card for each change:

```sh
cargo xtask build rpiz2
```

This builds normal `init`, generates an MBR/exFAT image containing
`init.elf`, includes its sparse representation in the kernel, and
produces `kernel8.img` for the existing single-payload `scip`
upload. Run this same command after changing init; no separate
preparation or SD-card copy is required. Pi 4 uses `rpi4` instead
of `rpiz2`. No protocol or chainloader changes are needed.

The Make upload wrapper is also available:

```sh
make chainboot BSP=rpiz2
```

The image is checked on every invocation; unchanged content does
not rewrite it. Init changes automatically refresh the embedded
filesystem. `cargo xtask boot-image rpiz2` is available separately
for image preparation or inspection.

Embedded filesystems are the default for xtask and Make. Use
`cargo xtask build rpiz2 --sd-card` or `EMBEDDED_FS=0` in Make
for the optional SD-card path. For direct Cargo builds, select
`embedded-fs` and first run `cargo xtask boot-image`. Chainloader
builds automatically exclude embedded filesystems.

`libraries/boot-image` uses a small sparse format rather than a
compression codec. A versioned header declares logical disk size
and record count; sorted records hold a 32-bit sector number and
512 literal bytes. All-zero sectors are omitted. The read-only
block device binary-searches records and returns zeros for absent
sectors, without allocating an expanded image or lookup index.
It rejects malformed lengths, duplicate/unsorted records, and
out-of-range sectors. No compression dependency or unsafe code
is added.

The current image is 512 KiB logically and 149,140 bytes in the
payload, about 146 KiB. Empty capacity consumes neither upload
bytes nor a full-size RAM buffer. The host builder grows images
by powers of two as needed, with a 64 MiB logical-size limit.
Occupied sectors remain literal; this is zero-sector omission,
not general compression. ELF allocation limits remain unchanged.

Hardware images live under `target/bootfs/<family>` and build
their executable in `target/init-embedded/<family>`. QEMU images
use separate directories and the test-only completion callback.
Both mount through the regular exFAT API and load `init.elf` with
the existing ELF loader.

Boot explicitly prints either `Init source: embedded sparse exFAT
filesystem` or `Init source: SD-card exFAT filesystem`. There is
no fallback between them, so stale files on the other source
cannot silently take over. Chapter 22 remains an optional
SD-card side-quest; subsequent chapters normally use the embedded
filesystem. The SD-card workflow remains available below.

## ELF loading inherited from Chapter 23

This chapter reads a static AArch64 executable from exFAT and
prepares an owned `ProgramImage` for process creation. Boot tries
the root-level file `init.elf` and prints its entry address and
loadable segments. Missing media, filesystem errors, and invalid
executables produce a warning and boot continues.

The loaded image is converted to `Process`, activated, and entered
through `ActiveProcess::run` when board wiring is supported.

## Implementation

`libraries/program-image` is a `no_std` library using `alloc` and
forbidding unsafe code. The kernel exposes it as
`libkernel::program`. Its ELF parser is
[`elf` 0.8](https://docs.rs/elf/0.8.0/elf/), with default features
disabled. Host tests exercise the same loader used by the kernel.

`program::load(filesystem, path, scratch)` borrows a mounted
`exfat_embedded::FileSystem` and reads the file without modifying
the disk. The caller owns the filesystem and sector scratch space.
The returned image owns its segment buffers, so the filesystem can
be dropped or reused immediately. Boot mounts locally; later
process creation can keep a filesystem and call the same function
repeatedly.

`ProgramImage::from_elf(bytes)` provides the parsing and preparation
step separately from disk access. The ELF header and program headers
are parsed using the crate's checked, fixed-endian interfaces.
Section headers, section names, and symbol tables are unnecessary
for loading and are ignored.

The result contains:

- `entry`: the initial instruction's virtual address.
- `segments`: loadable segments sorted by virtual address.
- Each segment's virtual address, ELF alignment, read/write/execute
  permissions, and owned data buffer.
- A buffer of exactly `p_memsz` bytes per segment. The first
  `p_filesz` bytes come from the file; the remainder is zero-filled
  for BSS and other uninitialized data.

The first implementation accepts ELF64, little-endian, AArch64
`ET_EXEC` files. It rejects PIE, relocatable objects, interpreters,
dynamic linking, TLS, and requests for executable stacks. The entry
must identify a complete, four-byte-aligned instruction in the
file-backed portion of an executable segment.

All segment ranges must fit in the file and the lower 48-bit virtual
address range; address zero is rejected. `p_filesz` must not exceed
`p_memsz`. Alignment must be zero, one, or a power of two, and file
offsets must agree with virtual addresses modulo that alignment.
Overlapping segment byte ranges are rejected. These checks follow
the [ELF program-header specification](https://refspecs.linuxfoundation.org/elf/gabi4%2B/ch5.pheader.html).

The loader caps files at 4 MiB, total segment storage at 8 MiB, and
the program-header count at 32. Allocations are fallible. The file
buffer is released after loading; peak storage includes both the
file and the segment buffers. These limits leave room in the
kernel's 16 MiB heap for its other users.

## SD-card workflow

Build the chapter's standalone Rust `init` program:

```sh
cd init
cargo build --release --features bsp_rpi3
cd ..
cargo run -p program-image --example inspect -- \
  init/target/aarch64-unknown-none-softfloat/release/init
```

`init` is a `no_std` Rust executable that directly blinks the
activity LED forever. It is excluded from the kernel workspace so its bare-metal
target and compiler wrapper do not affect host tools. Build from
its directory to apply its local Cargo configuration. Use
`bsp_rpi3` for Pi 3 and Zero 2 W, or `bsp_rpi4` for Pi 4. Without a
BSP feature, the compiler uses the generic AArch64 target CPU.

Copy `init/target/aarch64-unknown-none-softfloat/release/init` into
the root of the card's existing exFAT partition as `init.elf`, then
build and boot the normal kernel:

```sh
cargo xtask build rpiz2 --sd-card
make chainboot BSP=rpiz2 EMBEDDED_FS=0
```

Boot prints the executable's entry address and loaded segments.
The current release build reports entry `0x2101a0` and two segments;
addresses and sizes can change with the compiler and build options.
Host inspection also reports the prepared image page count,
currently two. Kernel process preparation additionally reserves
the stack/guard range and allocates four stack pages.
On a supported board, boot reports `Launching init` and the
activity LED should blink. IRQ-driven kernel console input stops
after launch because the initial process masks interrupts.
The Pi 3 and Pi 4 build selectors remain available.

For a separate fixture exercising writable data and BSS zeroing:

```sh
python3 tools/make-loader-fixture.py
cargo run -p program-image --example inspect -- \
  target/loader-fixture.elf
```

This hand-encoded ELF has a looping AArch64 instruction, an
executable segment, and a writable segment with initialized data
and a zero-filled tail. It reports entry `0x100b0` and two segments.

## Validation and remaining work

```sh
cargo test -p program-image
cargo test -p boot-image --all-features
make clippy BSP=rpi3
make clippy BSP=rpi4
make test BSP=rpi3
```

Host tests cover truncated input, unsupported executable formats,
invalid ranges and alignment, segment overlap, entry validation,
allocation limits, owned buffers, zero filling, and read-only exFAT
loading. The filesystem test generates a temporary executable on
a fresh in-memory exFAT card and removes it afterward, including
during assertion unwinding. It verifies removal and that the
loaded image survives both deletion and unmounting. Host loader
tests run in CI without a prebuilt ELF or physical SD card.

Sparse-image tests cover deterministic generation, exFAT mounting,
file contents, read-only enforcement, omitted sectors, malformed
records, and zero-cost empty capacity. QEMU runs the compiled init
from the sparse filesystem. Source-switching and incremental
Make checks verify that init changes refresh the image and that
each kernel includes only the selected boot source.

QEMU tests exclude the boot SD-card path because QEMU lacks
the VideoCore mailbox service used by card initialization.

The first process now runs without scheduling, exit, or privilege
isolation. Preemption, scheduling, and system calls remain future
chapters. The SD-card limitations described in Chapter 22 remain.

## Kernel documentation

From this chapter's root, generate private-item kernel docs:

```sh
cargo xtask doc
```

Documentation defaults to the Pi 3 family, including Zero 2 W.
Select a board and optionally open the generated docs:

```sh
cargo xtask doc rpiz2 --open
cargo xtask doc rpi4 --open
```

Additional Cargo documentation arguments, such as `--locked` or
`--offline`, are forwarded. The command selects the kernel package
and AArch64 target, skips dependency docs, and preserves existing
build artifacts. Normal build defaults and host tools are unchanged.

`make doc BSP=rpiz2` delegates to `cargo xtask doc rpiz2 --open`.

The equivalent direct Cargo command for Pi 3 and Zero 2 W is:

```sh
cargo doc --package mingo --no-deps \
  --target aarch64-unknown-none-softfloat \
  --features bsp_rpi3 --release \
  --document-private-items --open
```

`--open` uses Cargo's configured browser. On a remote/headless
machine, set `BROWSER` to a suitable browser command, or omit
`--open` and open the generated HTML yourself. The kernel library
starts at
`target/aarch64-unknown-none-softfloat/doc/libkernel/index.html`.
