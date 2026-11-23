# Backport and Make removal plan

Planning inventory, started 2026-11-16. On 2026-11-23 the ELF
kernel-loading section was committed and backported through
Chapters 15–24. Make removal and general test-command work
remain deferred.

## Scope and current state

- Cargo support exists from Chapter 01.
- `cargo xtask` first appears in Chapter 15.
- Backport xtask to the chapter where it should first appear,
  then carry it forward. Chapter 01 is the candidate if xtask
  takes over basic builds and image preparation.
- Introduce later capabilities at their original chapters;
  do not add future chapter machinery to earlier snapshots.
- The eventual goal is to remove Make, including helper
  makefiles and indirect calls from Rust tools.
- The experimental Chapter 24 `xtask test` change was reverted.
  There is no new test command to preserve or backport.

## Chapter rollout points

| Chapter | Existing workflow to support |
|---|---|
| 01 | Build/image, docs, QEMU, Clippy, clean, inspection |
| 03 | QEMU boot smoke test (`test_boot`) |
| 06 | Serial upload (`chainboot`) |
| 07 | Kernel/chainloader feature and output selection |
| 08 | JTAG bootstrap, OpenOCD, GDB and unoptimized GDB |
| 12 | Integration tests, combined test command, host Clippy |
| 15 | Existing xtask and translation-table tooling |
| 17 | Kernel symbol generation and ELF patching |
| 23 | Embedded filesystem preparation and source selection |
| 24 | Separately built init execution fixture for QEMU |

## Make removal inventory

- [ ] Move board selection, aliases, CPU flags, linker flags,
  and linker-script paths into xtask configuration.
  Preserve `rpiz2` as a Pi 3-family selector.
- [ ] Cover the full build pipeline: Cargo compilation,
  ELF payload/flat bootstrap packaging, and removal of offline
  symbol and translation-table patching as described below.
- [ ] Preserve normal kernel versus chainloader outputs and
  feature selection, including debug and filesystem variants.
- [ ] Replace Make's dep-info parsing and configuration stamps
  with reliable Cargo/xtask rebuild behavior. Check board and
  feature changes for stale artifacts.
- [ ] Move normal QEMU launch and board-specific arguments.
  Chapters 01–02 use instruction tracing; later chapters use
  serial stdio. Tests additionally enable semihosting.
- [ ] Move serial upload through `scip`, preserving configurable
  executable, serial device, payload, and transfer settings.
  Chapter 06 defaults to the previous chapter's kernel image;
  later chapters use the local kernel image.
- [ ] Move `jtagboot`, OpenOCD, GDB, and `gdb-opt0` workflows.
  Preserve board assets, configurable tools/interface, separate
  build directories, debug information, and unstripped ELFs.
  Coordinate entry paths with stage 2's runtime page-table
  construction and the ELF symbol-metadata contract.
- [ ] Move kernel and host Clippy commands, including their
  target, package, feature, and warning configurations.
- [ ] Preserve documentation generation and opening behavior.
  Later Make documentation targets already delegate to xtask.
- [ ] Move `readelf`, `objdump`, and `nm` inspection commands.
- [ ] Move cleanup of build directories and generated images.
- [ ] Keep embedded filesystem preparation, explicit SD-card
  selection, and chainloader exemption consistent across paths.
- [ ] Replace output formatting and size reporting from
  `common/format.mk` and `common/operating_system.mk` with Rust
  output code; retain useful behavior on Linux and macOS.
- [ ] Remove `kernel_symbols.mk` through the ELF symbol plan
  below. Both xtask build and the integration-test runner
  currently invoke it, so both callers need migration.
- [ ] Update CI, README commands, review checklists, and other
  Make references after replacement commands are validated.
- [ ] Remove obsolete Makefiles and common `.mk` helpers only
  after their workflows have replacements.

## Testing tasks

`cargo test --features=bsp_rpi3` selects board code but still
targets the host. The kernel is AArch64 bare-metal code, so
host compilation fails. Merely selecting the AArch64 target
also omits image preparation and QEMU runner configuration.

- [ ] Introduce `cargo xtask test` at the appropriate chapter
  and roll forward, starting with boot tests in Chapter 03.
- [ ] Add integration-test orchestration from Chapter 12,
  retaining a way to select one integration test.
- [ ] Run portable library and tool tests on the host as those
  packages appear; run kernel tests through QEMU.
- [ ] Build required host tools and configure the Cargo runner,
  QEMU machine/arguments, objcopy, and patcher paths explicitly.
- [ ] Exercise stage 2's runtime page-table construction and
  retained ELF symbols in boot and integration tests once
  the corresponding replacements are introduced.
- [ ] Prepare the separate QEMU init/filesystem fixture in
  Chapter 24 without replacing production init artifacts.
- [ ] Keep test builds and images separate from normal kernel,
  chainloader, filesystem, and hardware debug artifacts.
- [ ] Propagate failed builds and tests as command failures.
  Reject unsupported board/test combinations explicitly;
  the current Pi 4 setup has no QEMU machine configured.
- [ ] Verify incremental rebuilds and switches between boards,
  kernel/chainloader modes, tests, and filesystem sources.
- [ ] Validate each changed chapter's available build and test
  workflows before advancing later chapter snapshots.
- [ ] Preserve the distinction between QEMU validation and
  hardware acceptance, especially SD-card and physical LED I/O.

## ELF kernel loading and runtime symbols

Implemented, committed, and backported to the first affected
chapters. Chapter 15 introduces ELF boot and runtime table
construction while keeping identity mappings; Chapter 16 moves
the kernel into the higher half. Chapter 17 adds runtime ELF
symbol lookup. Later chapters inherit this implementation.
The display pins match the rewritten chapter tips. No changes
were made to Chapters 01–14 or their pins.

The replacement removes custom symbol generation and patching
in favor of retained ELF metadata. It supersedes the proposed
two-link approach with a generated linker script/data object.

The kernel is already linked at its intended virtual addresses.
An ELF loader copies loadable segments to their physical load
addresses. Stage 1 enters the physical bootstrap (currently
`0x80000`) with the MMU off. Stage 2 constructs its page tables,
enables the kernel mappings, and continues at linked virtual
addresses. Symbol values already contain those virtual
addresses; no function relocation, symbol-address rewriting,
or second symbol-data link is needed.

Ordinary `PT_LOAD` loading does not retain `.symtab` and
`.strtab`, since those tables normally lie outside loadable
segments. Keep them explicitly and make them accessible after
the MMU transition. Their storage address is independent of
the addresses described by their symbol records.

- [x] Extend stage 1 to accept a kernel ELF over UART rather
  than copying a flat payload directly to `0x80000`.
- [x] Validate the AArch64 ELF header, segment/file bounds,
  physical destinations, sizes, and physical bootstrap entry.
  Copy segment file bytes and initialize required zero tails.
  Preserve firmware park code in the first 64 KiB; stage 2's
  seven-page boot stack occupies physical 64–512 KiB.
- [x] Stage the ELF away from segment destinations and loader
  code/stack. Preserve the firmware DTB and prevent source or
  loader overwrite during loading. The existing chainloader
  relocation near 32 MiB is useful groundwork.
- [x] Retain `.symtab` and its associated string table, either
  in the source ELF buffer or as separate retained copies.
  Pass their locations and lengths alongside the DTB through
  an explicit boot-information contract.
- [x] Map and reserve retained metadata for the kernel's
  lifetime, including after the MMU transition and heap setup.
- [x] Build initial page tables in stage 2's physical bootstrap
  before enabling the MMU. Use linker-defined kernel boundaries
  and physical placement; stage 1 need not construct tables or
  supply the kernel's own mapping recipe.
- [x] Keep bootstrap code and data usable at physical addresses
  until translation is enabled. Reserve and align table storage,
  preserve intended permissions, and let the kernel own its
  tables for subsequent MMIO mappings and other updates.
- [x] Remove `translation_table_tool` and patched table/base
  placeholders after runtime construction covers all supported
  boot paths. No page tables are required before chainloading.
- [x] Replace custom symbol lookup with bounded ELF symbol
  lookup using linked virtual addresses and string offsets.
  Define handling of symbol types, aliases, zero sizes, and
  address ranges; do not assume ELF symbols are address-sorted.
- [x] Demangle Rust names when formatting diagnostics using a
  `no_std`-compatible demangler. Prefer allocation-free output
  so panic reporting does not depend on a functioning heap.
  Demangling is for readability, not execution or lookup.
- [x] Package upload ELFs without unnecessary DWARF sections,
  while retaining symbol/string tables and runtime unwind data.
  Keep the full debug ELF separately for GDB.
- [x] Keep a small flat firmware-facing bootstrap. For
  standalone boot, embed the kernel ELF in that image and run
  the same loader. This avoids depending on firmware ELF
  interpretation or adding a bootstrap filesystem driver.
- [x] Remove `kernel_symbols/`, `debug-symbol-types`, the host
  symbol generator/patcher, the fixed 32 KiB symbol reservation,
  patched symbol count, and `kernel_symbols.mk` once replaced.
- [x] Adapt QEMU and hardware-debug entry paths to the retained
  metadata contract and stage 2 bootstrap. Verify mappings,
  readable backtraces, and existing symbol sanity checks
  without offline table or symbol patching.
- [x] Backport ELF boot/runtime tables from Chapter 15 and ELF
  symbol consumption from Chapter 17 through Chapter 24.
  Preserve each chapter's feature boundaries and the linear
  parent chain. General xtask/Make/testing backports remain
  separate work.

Stage 1 loads the kernel and preserves boot metadata; stage 2
owns its memory mappings. ELF headers describe segment layout,
but the CPU still needs architectural page-table descriptors.
Constructing those descriptors at boot replaces the current
host-side translation-table patcher without changing the linked
virtual addresses or relocating kernel functions.

Validation passed: seven new parser tests, existing loader and
filesystem tests, xtask tests, strict host Clippy, both BSP
builds, and the full QEMU boot/five-integration-kernel suite.
UART QEMU testing transferred a real ELF and reached compiled
init execution; oversized and invalid uploads were rejected.
Runtime exceptions print demangled names from retained ELF.
The user confirmed the new ELF UART boot path on hardware.
Every affected chapter passes host tests, both BSP builds and
Clippy checks, and its full QEMU suite. Physical JTAG remains
unverified. Rewritten chapter branches require force-with-lease
when published; the display-tree main commit is a normal update.

Normal builds now produce `work/kernel.elf` for UART upload and
`work/kernel8.img` for standalone firmware boot. Build the new
serial receiver with
`cargo xtask build rpi3 --features chainloader` and install its
`chainloader8.img` on the card before uploading
ELF payloads. The old flat receiver is incompatible with them.

## Reference files

- `01-wait-forever/Makefile`: baseline build configuration.
- `03-hello-world/Makefile`: first boot test.
- `06-uart-chainloader/Makefile`: first serial upload workflow.
- `08-hw-debug-jtag/Makefile` and `tools/jtag/` within that
  chapter: hardware debugging commands and assets.
- `12-integrated-testing/Makefile`: integration orchestration.
- `work/Makefile`: current complete target inventory.
- `work/libraries/kernel-image/`: safe ELF load/symbol parser.
- `work/xtask/src/main.rs`: ELF build, packaging, and flat-helper
  wrapping commands.
- `work/tools/kernel_test_runner/src/main.rs`: test images use
  the same ELF bootstrap without offline patchers.
- `HISTORY-MAP.md`: rewritten chapter tips and recovery refs.
