# Flamingos project status

Updated 2026-10-29. This is the project-wide handoff, maintained
in the top-level display tree on `main`. Update it when the
active chapter, validation evidence, or next task changes. Do not
copy it into each chapter branch: chapter READMEs document their
snapshots.

## Repository layout and local state

- The repository root is the `main` display worktree. Numbered
  directories are detached submodule checkouts, pinned to chapters.
- `work/` is the linked development worktree, currently on
  `23-load-elf` at `cbfcfc67`.
- Chapters 01–21 now contain exactly one commit per chapter after
  the preserved upstream boundary, `644474cc`. Chapter 21 ends at
  `a92711bc`.
- Chapter 22, `f092179a`, retains five development commits after
  Chapter 21. Chapter 23 now has three after Chapter 22. The
  branches remain one linear sequence. Chapters beyond 21 may
  accumulate commits and can be squashed later.
- All 23 display pins and submodule checkouts match local chapter
  tips. The display tree records the history maintenance, policy,
  and current project handoff.
- The rewrite is local. Remote-tracking refs still identify the
  published history. Nothing has been pushed.
- `work/ELF-RESEARCH.md` remains an untracked research note.

Develop in `work/`, rather than in a detached chapter submodule.
See [HISTORY-MAP.md](HISTORY-MAP.md) for the old-to-new mapping.

## Current milestone

Chapters 01–21 establish the bare-metal kernel foundations:
console and drivers, virtual memory, exceptions and interrupts,
stable kernel tests, symbolic backtraces, heap allocation, timer
callbacks, firmware CPU discovery, and secondary-core startup
groundwork. They do not provide a process scheduler.

Chapter 22 provides the runtime SD-card and exFAT stack:

```text
exfat_embedded::FileSystem
          |
   SdCardBlockDevice
          |
 EMMC / EMMC2 single-sector PIO
          |
        SD card
```

Chapter 23 prepares owned program images from root-level
`init.elf`. The safe, `no_std` `program-image` library accepts
static little-endian AArch64 ELF64 `ET_EXEC` files. It validates
loadable ranges, alignment, permissions, entry instructions, and
allocation limits, then copies segment data and zero-fills BSS.
It rejects dynamic linking, PIE, TLS, and executable stacks.

The filesystem is borrowed while loading. The returned image owns
its buffers, so the mount can be dropped or reused. Boot retains
the image and logs its entry and segments; errors warn and boot
continues. The standalone Rust `init` contains a looping entry.

**Programs are loaded, not executed.** There are no process page
tables, process stack, initial CPU context, or runnable process.
See [Chapter 23](23-load-elf/README.md).

## Next task

Turn `ProgramImage` into a process:

1. Define page backing, virtual mappings, and permission policy.
2. Compose or reject ELF segments that share a page.
3. Allocate a stack and construct the initial CPU context.
4. Define the initial execution privilege and trap contract, then
   demonstrate a controlled launch before adding scheduling.

EL0 execution additionally needs TTBR0 configuration, user
mapping attributes, and lower-EL exception/IRQ handling. Today
TTBR0 walks are disabled and lower-EL traps reach the default
panic handler.

Preemption, scheduling, userspace separation, and system calls
remain later milestones in [EXTENSION.md](EXTENSION.md).

## Validation

The October 29 review passed, before the history rewrite:

- Ten `program-image` host tests and two rustdoc examples.
- The `xtask` host test.
- The Rust `init` release build and loader inspection: entry
  `0x2101a0`, with two segments.
- Chapter 23 QEMU boot and all five integration tests.
- Chapter 23 Clippy for Pi 3 and Pi 4.

After the rewrite and the kernel-test repairs, all 25 executable
checks passed with Rust 1.99.0:

- Pi 3 and Pi 4 builds for Chapters 01–02: four checks.
- QEMU boot tests for Chapters 03–11: nine checks.
- QEMU boot/integration suites for Chapters 12–23: twelve checks.

Every chapter passed the parent-chain, source-scope, pin, and CI
YAML checks. The changed Rust files passed formatting checks.

The snapshot audit found only CI changes and two narrow classes
of repairs required by the wider executable checks:

- Chapter 12 qualifies its boot-test CPU helper.
- Chapters 14–21 update existing kernel-test exports and explicit
  unsafe blocks for Rust 2024. The user approved those syntax
  changes; they introduce no new unsafe operations.

At the recuration snapshot, other source content, host loader
tests, fixtures, and dependencies matched the pre-maintenance
tips. The old 228-check cleanup matrix in `HISTORY-MAP.md` is
historical evidence and was not rerun in full for this maintenance.

Subsequent Chapter 23 work enabled host loader tests in CI. Its
filesystem test generates `loader-test.elf` on a fresh in-memory
exFAT card, keeps loader reads write-protected, and removes and
flushes the file afterward. Cleanup also runs during assertion
unwinding. The test verifies deletion and image ownership after
filesystem teardown; no physical card is modified.

All ten loader tests and two rustdoc examples passed from a fresh
target directory without prebuilt ELF artifacts. Strict Clippy
for all loader targets and workspace formatting checks also passed.

QEMU test builds skip SD-card initialization and disk ELF loading
because the needed VideoCore mailbox service is unavailable.
Passing QEMU tests do not establish hardware ELF loading.

## Hardware and storage boundary

The previous handoff records successful Zero 2 W card
initialization, sector-zero reads and matching CRC, exFAT mount,
lookup/read of `123.txt`, and correct empty-slot reporting. It
also records use of `/dev/ttyUSB0` with `make chainboot`. Pi 3
and Pi 4 storage paths and physical OpenOCD/JTAG operation remain
unconfirmed. End-to-end hardware ELF loading is not recorded.

Storage remains blocking, single-sector PIO without DMA,
multi-block transfers, media monitoring, or general error
recovery. The write path exists but lacks on-card
destructive/power-loss testing; higher layers remain read-only.
SDUC/CSD v3 is deliberately unsupported. These are deferred work,
not prerequisites for the current read-only loader.

## Build and CI workflow

The supported board selectors are `rpiz2`, `rpi3`, and `rpi4`.
Zero 2 W uses the Pi 3 family. From `work/`:

```sh
cargo xtask build rpiz2
make clippy BSP=rpi3
make clippy BSP=rpi4
make test BSP=rpi3
make chainboot BSP=rpiz2
```

Build `init` from its own directory to apply its Cargo settings:

```sh
cd init
cargo build --release --features bsp_rpi3
```

Chapter CI runs on pushes/PRs without the inherited `master`
filter. That selector came from upstream's January 2020 workflow
and survived the later stable-test conversion. Chapters 01–02
build; 03–11 also run boot tests; 12–23 run boot and integration
tests. Obsolete early Ruby jobs were removed. Chapter 23 also runs
`cargo test --locked -p program-image`, using its generated,
temporary in-memory exFAT executable.

`make doc` still needs a kernel-only package selection so the
bare-metal target is not applied to host tools. A known bare-URL
rustdoc warning also remains.

## Kernel test unsafe investigation

Investigation only: no kernel-test refactoring was implemented.

- The symbol test's unsafe function signature is redundant; its
  body uses safe APIs. Several other tests have unnecessarily
  broad unsafe blocks around safe assertions.
- A shared entry macro and minimal boot setup could leave ordinary
  test bodies safe while retaining the existing harness-free
  Cargo/QEMU workflow. Setup must enforce once-only, single-core
  initialization with valid mappings/vectors and masked IRQs.
- The exported entry still needs its linker contract, and the SVC
  test still needs architecture support. Centralizing those
  obligations does not eliminate the underlying unsafe boundary.
  Preserve the nested call that tests link-register restoration.
- The deliberate page-fault `read_volatile` violates Rust's
  documented requirement that volatile accesses must not trap.
  Plan a reviewed architecture fault-injection helper rather than
  hiding that read in a safe wrapper. This remains unfixed.

## Preservation and working rules

Before rewriting, all 23 chapter tips and `main` were preserved
under local `archive/2026-10-29-pre-recuration/*` branch refs. A
verified recovery bundle and the original handoff note are in
`.git/recuration-2026-10-29/`. This bundle is local to this
checkout; the earlier independent recovery artifacts remain
documented in `HISTORY-MAP.md`.

- Never push without explicit permission. Publishing this rewrite
  requires coordinated chapter updates before publishing pins.
- Never commit unrelated work without an explicit request.
- Agent commit subjects start with a lowercase past-tense verb
  and include AI attribution.
- Do not add unsafe code without explicit approval.
- Preserve untracked research notes and unrelated user changes.
