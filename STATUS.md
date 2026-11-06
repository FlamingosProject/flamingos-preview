# Flamingos project status

Updated 2026-11-05. This is the project-wide handoff, maintained
in the top-level display tree on `main`. Update it when the
active chapter, validation evidence, or next task changes. Do not
copy it into each chapter branch: chapter READMEs document their
snapshots.

## Repository layout and local state

- The repository root is the `main` display worktree. Numbered
  directories are detached submodule checkouts, pinned to chapters.
- `work/` is on `24-run-init`, based on `e5a37b6e`. Chapter 24
  is committed at `77686a38` with its display submodule.
  The restored stash `06bc7024` remains an already-applied
  recovery copy; do not apply it again. The LED investigation
  remains open, with hardware work deferred to the user.
- Chapters 01–21 now contain exactly one commit per chapter after
  the preserved upstream boundary, `644474cc`. Chapter 21 ends at
  `a1baa57c`.
- Chapter 22, `815e935e`, retains five development commits after
  Chapter 21. Chapter 23 now has four after Chapter 22. The
  branches remain one linear sequence. Chapter 24 adds one
  commit after Chapter 23. Chapters beyond 21 may
  accumulate commits and can be squashed later.
- Chapters 01–22 display pins and checkouts are unchanged.
  Chapter 23 now points to `e5a37b6e`, one new local commit beyond
  recorded `origin/23-load-elf`; it has not been pushed.
- Earlier rewritten branches were published by the user. The
  Chapter 23 backport, Chapter 24 snapshot, and display updates
  are local until the user pushes.
- `work/ELF-RESEARCH.md` remains an untracked research note.
- The user's top-level `config.txt` remains untracked; the
  reviewed card configuration is not part of the tutorial code.

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
`init.elf`, using an embedded sparse exFAT image by default.
Its optional SD-card path uses `--sd-card` in xtask or
`EMBEDDED_FS=0` in Make. The safe, `no_std` `program-image` library accepts
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

## Chapter 24 snapshot and validation

Chapter 24 implements owned page backing, a guarded process
stack, private TTBR0 mappings, activation/teardown, cache
synchronization, and the EL1h transfer into trusted init. Kernel
and MMIO mappings remain accessible. Init uses the shared raw
LED helper and physical-counter delays. Scheduling is deferred.
The README explicitly records unresolved physical LED behavior.

Fresh November 5 precommit checks passed: 15 loader/backing host
tests, three boot-image tests, three xtask tests, two rustdoc
examples, formatting, both BSP kernel Clippy checks, strict host
Clippy, and the full QEMU boot/five-integration-kernel suite.
QEMU executes the separately built init and validates its stack,
EL1h state, timer delay, and both GPIO-shaped writes. Independent
static review found no actionable issue. Zero 2 W and Pi 4
normal builds and the Zero 2 W boot-trace chainloader build pass
(the chainloader retains existing warnings). Physical LED operation
is still unverified. Build evidence is under
`work/target/chapter24-commit-*.log`.

The historical investigation below records observations and
intermediate local state before this snapshot was committed.

## LED diagnostic in progress

Paused November 5 at the user's request. The original January 27
kernel also did not visibly light ACT. The microscope photo in
`/tmp/led.jpg` shows the intended component; there is no conclusive
visual diagnosis. Tomorrow the user plans a better microscope
inspection, diode testing, possibly direct LED powering, and
preparing another Pi. Chapter 24 sources have been restored;
no further code diagnosis or hardware testing is underway.
The commit drill rebuilt Chapter 24 kernel and boot-trace
chainloader images, replacing the historical test artifacts.

Important correction: the agent previously misidentified the LED
as the component between the USB sockets. An explicitly annotated
Zero 2 W photograph places ACT beside the camera connector,
immediately above the mounting hole next to the power USB port
(GPIO header at top, USB sockets at bottom). Earlier no-light
reports may have observed the wrong component. Ask the user to
check this actual LED with the already-built original payload
before drawing hardware-failure conclusions. Photo:
https://i.ebayimg.com/images/g/oxAAAOSw1DhnZA6H/s-l1600.jpg

History review also found the Jan 27 xtask drops `boot_trace`
during its second Cargo invocation, and checks launch success
rather than child exit status. The last original-kernel build
bypassed that wrapper and its enabled blink calls were verified.
The original boot sequence also blinks before filtering other
cores, which can cause races if they enter together. No GPIO
pin/address correction explaining the current symptom appeared
in the nearby February/March commits.

The user also saw no light with historical chainloader `e49ca847`.
Built the original regular kernel from `e47c1480` (January 27),
which introduced LED tracing, as the next chainload payload.
`work/kernel8.img` contains the original startup pattern 9 twice,
followed by stages 1-4 and Rust stage 8. Disassembly confirms
trace enable constants and calls; translation tables were patched
with that commit's host tool. No historical source was modified.

That commit's old xtask drops `boot_trace` during its copy step,
so the final artifact was built explicitly with:
`cargo build --locked -p mingo --release`
`--target aarch64-unknown-none-softfloat`
`--features bsp_rpi3,boot_trace`, followed by copying the raw ELF,
patching it with `tools/bin/translation_table_tool rpi3`, and
`rust-objcopy --strip-all -O binary` to `kernel8.img`.
SHA-256: `d4e3f3ae47eda3d0255e4d37baadf075a214996cab7c91d73e50a8f4c6aed887`.
The Chapter 24 stash `06bc7024` remains intact. Hardware testing
of this original kernel payload also found no visible LED light.

The historical LED-enabled chainloader also failed to light ACT.
Reviewed the user's top-level `config.txt`: no active LED/GPIO29
overrides, overlays, includes, or custom kernel filename. JTAG
is enabled on GPIO22-27, which does not select GPIO29. Explicit
GPIO directives are commented out. No culprit was identified;
`config.txt` was left unchanged.

November 5 historical test: Chapter 24 changes were stashed with
`--include-untracked` at the user's request. LED tracing originally
appeared in `e47c1480` in a regular Chapter 20 kernel. The first
historical Chapter 06 chainloader snapshot with that tracing is
`e49ca847`; this was built unchanged using:
`cargo build --locked --release --features bsp_rpi3,boot_trace`.
Then `rust-objcopy` produced `work/chainloader8.img` (12112 bytes).
Disassembly confirms enabled stage 1, 2, and 3 blink calls. Copy
it to the SD boot partition using the configured kernel filename.
The user will test it and retrieve `config.txt` to check pin
configuration. No source changes or commits were made in the
historical checkout. Generated artifacts may appear untracked
because the older ignore rules differ.

November 5: the user suspects the older SD bootstrap is affecting
LED behavior. Built the current Chapter 24 Zero 2 W chainloader
with `cargo xtask build rpiz2 --features chainloader,boot_trace`.
The output is `work/chainloader8.img` (2024 bytes). Disassembly
confirms enabled boot-stage blink calls. Copy it to the SD boot
partition under the filename selected by `config.txt` (normally
`kernel8.img`). Hardware testing is pending; the agent did not
write the card. Existing chainloader warnings remain.

The user reports correct GPIO function and level readbacks, but
no physical LED light. The user does not recall whether it ever
lit; the physical LED path remains unresolved.

At the user's request, boot debugging, the kernel diagnostic,
and init now share the raw GPIO implementation extracted from
`led_debug.rs`, in `kernel/src/_arch/aarch64/cpu/led.rs`. This
unconditional helper selects output mode, then writes SET/CLR.
Early debug passes a physical base; post-MMU callers pass the
mapped GPIO alias. Duplicate GPIO-driver diagnostics and init
MMIO writes were removed. The kernel still holds on, off, on
for one second each before loading init, logging `Kernel debug
ACT`. New unsafe boundaries have Safety/Need documentation;
the user explicitly requested reuse of the raw debug helper.

The shared implementation passes QEMU init execution and Pi 4
Clippy; Zero 2 W normal embedded and boot-trace chainloader
builds pass (the latter retains existing warnings). Physical
results also show no light with the shared debug helper.
The next hardware check enables `boot_trace` to exercise the
physical GPIO path before the MMU transition:
`cargo xtask build rpiz2 --features boot_trace`.
The existing BSP-conditional early helper now uses Pi 4 Model B
GPIO42 active-high; Zero 2 W remains GPIO29 active-low. Early
Pi 3-family tracing assumes Zero 2 W direct wiring. Changes
remain uncommitted.

## Next task

The user confirmed real-hardware ELF loading on November 2.
Chapter 24 is `24-run-init`: implement the following incrementally,
with privileged access to kernel and hardware mappings. Its final
demonstration will blink the LED directly from `init`.

1. Prepare page backing, virtual mappings, and permission policy.
   Current work uses fallibly allocated, zero-padded 64 KiB pages,
   rejects segments sharing a page, and constructs inactive tables
   for the lower 1 GiB. TTBR1 kernel/MMIO aliases remain intact.
   No new unsafe code is introduced. Activation is deferred.
2. Allocate a process stack and construct the initial CPU context.
   Current work adds four zeroed RW/XN pages at the top of the
   lower 1 GiB, an unmapped guard immediately below, and a reserved
   range excluding ELF segments. `Process` owns its memory and an
   AArch64 context: entry PC, aligned stack-top SP, zero x0–x30,
   and saved status `0x3c5` (EL1h, DAIF masked). This remains
   inactive metadata; no registers or translation state change.
3. Enable the lower-address translation needed by `init`.
   Current work adds `Process::activate` and a core-bound owning
   guard. TTBR0 uses 64 KiB pages, T0SZ=34, and matching cacheable
   walk attributes. TTBR1 kernel/MMIO mappings are preserved.
   Drop disables aliases and restores the prior translation
   state before backing is released. Hardware AT/PAR inspection
   supports nontrapping translation/permission checks.
4. Launch `init` and blink the LED to demonstrate execution.
   Implemented: high-alias DC CVAU, low-alias IC IVAU, barriers,
   register restore, and nonreturning ERET to the process stack.
   The active guard is retained permanently. Init receives GPIO
   alias/pin/polarity in x0–x2 and directly blinks with 250 ms
   phases using the physical counter. Recognized hardware is
   Zero 2 W (GPIO29 active-low) and Pi 4 B (GPIO42 active-high).
   Pi 3 B uses firmware virtual GPIO, so unknown/unsupported
   wiring skips the blink launch rather than guessing a pin.

The user explicitly approved the scoped Step 3 and Step 4 unsafe
boundaries, including cache/context transfer and init GPIO MMIO.
Every new unsafe declaration/block has Safety and Need explanations.
All four implementation steps are complete; physical LED acceptance
remains pending. Preemption and scheduling remain later work.

The user subsequently reported that the hardware LED did not blink.
Before adding the requested kernel-LED-on diagnostic, we are
removing manual SD-card executable updates from the development
loop. `EMBEDDED_FS=1` now builds normal init, packages it into a
sparse exFAT image, and embeds it in the normal kernel upload.
No DEFLATE codec is used: the user requested omission of empty
space only. Sorted nonzero sector records are read in place;
omitted sectors return zeros, with no expanded disk allocation.
Current payload: 149,140 bytes for a 512 KiB logical image.
Logical capacity can grow to 64 MiB without allocating all of it
in the kernel. The embedded path is now the default in both xtask and Make.
The SD-card path is explicit, with no fallback between sources.

The kernel-LED-on diagnostic has not yet been added. Physical LED
behavior remains unresolved; QEMU only verifies the execution
and register-shaped-memory path.

EL0 execution would additionally need user mapping attributes
and lower-EL exception/IRQ handling. Current process descriptors
permit only EL1 access, and lower-EL traps still reach the default
panic handler.

Preemption, scheduling, userspace separation, and system calls
remain later milestones in [EXTENSION.md](EXTENSION.md).

## Validation

November 2 default/backport maintenance:

- The user authorized committing only the Chapter 23 backport and
  its display pin, leaving Chapter 24 process work uncommitted.
- Chapter 23 commit `e5a37b6e` adds the sparse filesystem and
  default embedded build path. Its init is still the original
  loop; no process mappings, context transfer, or LED code was
  backported. Chapter 22 is retained as an optional side-quest.
- Chapter 24 was advanced to that new base without changing its
  working files. A complete source snapshot, diff, and old refs
  are preserved in `.git/embedded-backport-2026-11-02/`.
- Both chapters pass default embedded builds, explicit SD-card
  selection, host tests, and QEMU boot/integration suites.
  Chapter 23 additionally passed both BSP Clippy/build checks
  and kernel docs. Its QEMU boot mounts the embedded image and
  reports the ELF entry/segments without launching it.
- xtask defaults tests cover embedded selection, the consumed
  `--sd-card` override, feature merging, and the chainloader
  exception. Plain Make defaults to embedded images; chainloader
  builds retain their independent bootstrap automatically.
- xtask now invalidates Make's build-configuration markers after
  writing a kernel variant. This fixes stale raw-ELF reuse when
  switching from an xtask chainloader build to a normal Make
  build; that sequence was tested in both chapters.

Sparse development-filesystem checks passed:

- Three host image tests: deterministic exFAT generation/read,
  read-only bounds, malformed record rejection, and omitted
  capacity reading as zeros. No new codec dependencies remain.
- Fifteen loader/backing tests, two rustdoc examples, xtask test,
  and Clippy for sparse image and both kernel BSPs.
- QEMU boot and all integration tests with EMBEDDED_FS=1: sparse
  image mount, ELF load, actual init execution, timer delay, and
  GPIO-shaped writes succeed.
- Embedded kernel builds for Zero 2 W and Pi 4, plus kernel docs.
- A reversible init-code change refreshes the image through the
  default Make target; restoring the code reproduces the original
  image bytes. Source switching produces a kernel with only the
  selected embedded or SD-card source string.

The user's primary hardware path is xtask. Use
`cargo xtask build rpiz2`: it builds init,
generates the image, and embeds it before building `kernel8.img`.
No separate preparation or SD-card copy is needed. The optional
Make wrapper is `make chainboot BSP=rpiz2`. Select SD explicitly
with `cargo xtask build rpiz2 --sd-card` or `EMBEDDED_FS=0`.
Direct Cargo builds require a preceding
`cargo xtask boot-image rpiz2`. QEMU init images and normal images
are isolated under `target/bootfs-qemu` and `target/bootfs`, with
separate executable build directories. None of this has been
uploaded to hardware by the agent; those changes were then
uncommitted.

November 2 Step 4 checks passed:

- Fifteen host tests, two rustdoc examples, and the xtask test.
- Kernel Clippy and normal kernel builds for both Pi families;
  Zero 2 W chainloader build and private-item kernel rustdoc.
- Normal init builds and strict Clippy for both CPU selections;
  the qemu_test variants also build and pass strict Clippy.
- QEMU boot executes the actual separately compiled Rust init
  after the previous memory/activation tests. It uses retained
  GPIO-shaped RAM and a trusted completion callback to verify
  output-mode configuration, both LED writes, low process SP,
  EL1 state, and masked exceptions. The fixture also uses the real
  physical-counter delay: its on/off cycle takes about 0.50 s.
  All five integration kernels
  still pass. QEMU does not prove physical LED behavior.
- Independent Step 4 review found no actionable cache, ERET,
  ownership, ABI, or alias bugs. Notes are in
  `work/target/step4-review.md`.

`cargo xtask test-init [board]` builds the QEMU fixture into
`target/init-qemu/<family>`, without overwriting the normal
`init/target` executable; a SHA-256 comparison across the test
run confirms the production artifact is unchanged. xtask test
builds and Make integration
tests prepare it before compiling the test kernel. Direct Cargo
test_build users must prepare the fixture first. Never copy the
qemu_test artifact to the physical card.

At the user's request, progress-report and Safety/Need rules were
merged into global `/home/bart/AGENTS.md` (also reached through
the `.codex/AGENTS.md` symlink); the temporary project copy was
removed. Chapter 24 and STATUS changes were uncommitted at that point.

November 2 Step 3 checks passed:

- Fifteen host tests, two rustdoc examples, strict host Clippy,
  Pi 3/Pi 4 kernel Clippy, formatting, and whitespace checks.
- Zero 2 W and Pi 4 kernel builds; private-item kernel rustdoc.
- QEMU boot and all five integration tests. New boot checks use
  AT/PAR for real translations and permission faults, assembly
  for owned low-alias reads/writes, and confirm the stack guard
  and kernel/MMIO mappings. They reject a second simultaneous
  activation and replace the first root with different physical
  backing at the same virtual address, then verify teardown
  restores TTBR0, TCR, TTBR1, and exception masks.
- An independent read-only review found no actionable Step 3
  issues under the current parked-core, kernel-execution-only
  constraints. Notes are in `work/target/activation-review.md`.

Real-hardware activation and process execution have not been
claimed or tested; the user previously confirmed hardware ELF
loading only. Global rules and current state are described above.

November 2 Step 2 checks passed:

- Fifteen host loader/backing/stack tests and two rustdoc examples.
- Strict host Clippy, Pi 3/Pi 4 kernel Clippy, formatting, and
  whitespace checks; public loader documentation generation.
- Zero 2 W and Pi 4 kernel builds.
- QEMU boot and all five integration tests. The boot check now
  verifies stack backing and RW/XN attributes, zeroed pages,
  unmapped guard boundaries, reserved-range rejection, and the
  initial context's PC, aligned SP, x0–x30, and saved EL1h status.

No new unsafe code or hardware-state changes were introduced.
Steps 1 and 2 were then uncommitted, pending user review.

November 2 Step 1 checks passed:

- Thirteen host loader/backing tests and two rustdoc examples.
- Strict Clippy for all host loader targets and kernel Clippy
  for Pi 3 and Pi 4; workspace formatting and diff checks.
- Zero 2 W and Pi 4 kernel builds.
- The Rust `init` build and host backing inspection: entry
  `0x2101a0`, two aligned pages.
- QEMU boot and all five integration tests. The new boot check
  verifies inactive mappings, physical backing, permissions,
  offsets, and a segment crossing the 512 MiB table boundary.

These checks validate preparation, not active process execution.
The pre-existing page-fault test intentionally reports a panic
and exits successfully when its expected diagnostics occur.

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
unconfirmed. The user subsequently confirmed end-to-end hardware
ELF loading on November 2.

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

From Chapter 15, where `xtask` is introduced, use
`cargo xtask doc [board] [cargo doc args]`. The command has been
backported to Chapters 15–22; Chapter 23 inherits the same support.
Chapters 01–14 are unchanged.

The command defaults to Pi 3/Zero 2 W, selects `mingo` and the
AArch64 target, includes private items, and skips dependency docs.
Pass `--open` to open the docs. `make doc BSP=...` delegates to
that command with `--open`; additional features and Cargo arguments
are forwarded. Build artifacts and normal Cargo defaults are
preserved. The bare-URL warning is fixed from Chapter 15 onward.

The backport is folded into the Chapter 15 commit and propagated
through the later snapshots. Chapters 15–21 still each have one
commit; Chapters 22 and 23 retain five and three development
commits. Their rewritten tips are recorded in `HISTORY-MAP.md`.

All eight backported chapters passed six checks apiece: xtask
tests, strict Clippy, workspace formatting, default docs, Zero 2 W
docs, and Pi 4 docs through `make doc`. Rustdoc warnings were denied
and both browser-opening paths were verified at each chapter.
Earlier chapter refs, pins, and contents were checked unchanged;
the snapshot diffs are limited to the four documentation files.

Previous tips are preserved locally under
`archive/2026-10-29-pre-doc-backport/*`, with a verified bundle at
`.git/doc-backport/before.bundle`. The user subsequently published
the chapter tips; their recorded `origin` refs match locally.

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

- Never push without explicit permission. For future history
  rewrites, publish chapter updates before publishing their pins.
- Never commit unrelated work without an explicit request.
- Agent commit subjects start with a lowercase past-tense verb
  and include AI attribution.
- Do not add unsafe code without explicit approval.
- Preserve untracked research notes and unrelated user changes.
