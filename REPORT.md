# Flamingos Preview Revision Report

## Repository Structure

This repository is not a conventional monorepo. The top-level checkout is a
container for submodules, and the numbered directories are successive snapshots
of the same upstream project:

- Each directory from `01-wait-forever` through `21-second-core` is a git
  submodule.
- Every numbered submodule points at
  `FlamingosProject/flamingos-preview.git`.
- The submodule branch name matches the directory name, so each directory is a
  branch-based revision of the same software rather than an independent package.
- The sequence is cumulative: each revision extends, reorganizes, or documents
  the previous one.

The software itself is a Rust `no_std` tutorial kernel for Raspberry Pi 3 and
Raspberry Pi 4, derived from the Rust Raspberry Pi OS tutorial lineage. It
starts as a minimal AArch64 boot skeleton and grows into a kernel with UART
I/O, drivers, timers, privilege-level transition, virtual memory, exception and
IRQ handling, integrated tests, symbol lookup, heap allocation, timer
callbacks, and early multicore boot work.

`X1_JTAG_boot` is present in the worktree, but it is not listed in
`.gitmodules`; it appears to be an extra checkout on `main`, not part of the
numbered submodule sequence.

## Revision Summaries

### 01-wait-forever

- Branch: `01-wait-forever`
- Checked-out commit: `fe7f54ea`
- Establishes the initial Rust embedded kernel skeleton.
- Adds a minimal AArch64 boot path that parks/hunts all cores in a wait loop.
- Provides the first linker script, BSP layout, Cargo package, and QEMU/build
  entry points.

### 02-runtime-init

- Branch: `02-runtime-init`
- Checked-out commit: `f3d1edaf`
- Extends `boot.s` so the assembly entry point can call Rust for the first
  time.
- Adds early runtime initialization before entering Rust code.
- Introduces CPU/BSP helpers and a panic path that halts execution.
- Adds the `aarch64-cpu` dependency for architecture register access.

### 03-hacky-hello-world

- Branch: `03-hacky-hello-world`
- Checked-out commit: `7bdfadfd`
- Adds early global `print!`/`println!` support for printf-style debugging.
- Introduces a console abstraction and Raspberry Pi console implementation.
- Uses a QEMU shortcut for UART output before the real UART hardware is fully
  configured.
- Improves panic output so failures become visible over the console.

### 04-safe-globals

- Branch: `04-safe-globals`
- Checked-out commit: `47fb88a1`
- Adds a synchronization module with a pseudo-lock primitive.
- Reworks global console access to be safer under Rust's aliasing rules.
- Demonstrates the first OS-style synchronization abstraction for mutable global
  kernel state.
- Cleans up printing macros and panic behavior around the locked console.

### 05-drivers-gpio-uart

- Branch: `05-drivers-gpio-uart`
- Checked-out commit: `aecbe8df`
- Adds real Raspberry Pi GPIO and PL011 UART drivers.
- Introduces a driver manager and BSP-owned driver registration.
- Adds MMIO address definitions for Raspberry Pi boards.
- Uses `tock-registers` for typed register access.
- This is the first revision intended to run usefully on real Raspberry Pi
  hardware over UART, not only under QEMU.

### 06-uart-chainloader

- Branch: `06-uart-chainloader`
- Checked-out commit: `bafa3c3a`
- Adds UART chainloading support so later kernels can be loaded over serial
  instead of repeatedly copying binaries to an SD card.
- Adjusts boot/linker behavior for the chainloader flow.
- Reworks console initialization and print handling around the loader model.
- Preserves the device tree handoff needed by the chainloaded kernel.

### 07-timestamps

- Branch: `07-timestamps`
- Checked-out commit: `dd935090`
- Adds the time subsystem and an AArch64 architectural timer implementation.
- Replaces cycle-count GPIO delays with timer-based delays.
- Annotates UART output with timestamps.
- Adds a `warn!` logging macro.
- Simplifies earlier boot assembly now that timer abstractions are available.

### 08-hw-debug-jtag

- Branch: `08-hw-debug-jtag`
- Checked-out commit: `655eb3c7`
- Primarily expands documentation and build targets for hardware debugging.
- Adds JTAG boot/debug workflow documentation using OpenOCD and GDB.
- Adds generated HTML README output for the tutorial.
- Makes only small code/configuration changes relative to the timestamped
  kernel.

### 09-privilege-level

- Branch: `09-privilege-level`
- Checked-out commit: `5a284d62`
- Adds early boot logic to transition from AArch64 EL2 hypervisor mode to EL1
  kernel mode.
- Introduces the first exception-related modules and asynchronous exception
  scaffolding.
- Updates boot register setup and exception return mechanics for the EL
  transition.
- Reports the current privilege level from the kernel.

### 10-virtual-mem-part1

- Branch: `10-virtual-mem-part1`
- Checked-out commit: `027293c6`
- Turns on the MMU.
- Adds generic and AArch64-specific memory/MMU modules.
- Introduces static 64 KiB translation tables.
- Uses a mostly identity-mapped address space while remapping the UART for
  demonstration.
- Adds BSP memory layout and MMU setup code.

### 11-exceptions-part1

- Branch: `11-exceptions-part1`
- Checked-out commit: `45a67766`
- Adds groundwork for architectural CPU exception handling.
- Introduces the AArch64 exception vector assembly and Rust exception reporting.
- Adds unwind/backtrace support through the `unwinding` crate.
- Demonstrates synchronous exception handling with MMU page-fault cases.
- Expands panic diagnostics with richer CPU/system state output.

### 12-integrated-testing

- Branch: `12-integrated-testing`
- Checked-out commit: `0692276d`
- Restructures the project into a Cargo workspace.
- Moves the kernel into `kernel/` and introduces `libraries/test-*` crates.
- Adds custom unit and integration test infrastructure for QEMU-based tests.
- Supports UART-style test I/O expectations.
- Keeps the boot test flow while adding richer automated test targets.

### 13-exceptions-part2

- Branch: `13-exceptions-part2`
- Checked-out commit: `785a1bc1`
- Adds peripheral IRQ handling.
- Implements interrupt controller drivers for Raspberry Pi 3's Broadcom
  controller and Raspberry Pi 4's ARM GICv2.
- Introduces the `IRQManager` abstraction and null IRQ manager fallback.
- Adds UART receive IRQ handling.
- Adds SMP/core identity scaffolding and more robust kernel state management.

### 14-virtual-mem-part2

- Branch: `14-virtual-mem-part2`
- Checked-out commit: `bbe8a65f`
- Makes virtual memory mapping more selective.
- Moves away from identity mapping the whole board address space.
- Adds lazy remapping of required MMIO ranges into a reserved virtual address
  region.
- Introduces richer MMU types, mapping records, and page allocation support.
- Prepares the design for eventual separation of kernel and user address
  spaces.

### 15-virtual-mem-part3

- Branch: `15-virtual-mem-part3`
- Checked-out commit: `d229ba44`
- Adds infrastructure for precomputed translation tables.
- Introduces `tools/translation_table_tool` and an `xtask` helper.
- Changes the build flow so translation tables are generated after compiling
  the kernel ELF and patched into the binary.
- Adds linker support for virtual address space sizing.
- Still identity maps the kernel, but removes the need to compute all kernel
  tables at runtime.

### 16-virtual-mem-part4

- Branch: `16-virtual-mem-part4`
- Checked-out commit: `8e57905f`
- Moves the kernel to the high end of the 64-bit virtual address space.
- Updates boot code, linker layout, MMU setup, and translation table generation
  for a higher-half kernel.
- Adds or restores a set of integration tests for console, timer, exceptions,
  and IRQ behavior.
- Adjusts the translation table tool to account for the kernel offset.

### 17-kernel-symbols

- Branch: `17-kernel-symbols`
- Checked-out commit: `4023d4ea`
- Adds kernel symbol lookup support for debugging.
- Introduces a `kernel_symbols` build artifact and linker description.
- Adds `libraries/debug-symbol-types`.
- Adds `tools/kernel_symbols_tool`, including a Rust implementation of the
  symbol extraction/generation flow.
- Hooks symbol generation into Makefile and `xtask` support.

### 18-kernel-heap

- Branch: `18-kernel-heap`
- Checked-out commit: `134dccc5`
- Adds kernel heap allocation.
- Enables `extern crate alloc` use in the kernel.
- Adds `memory::heap_alloc` and heap memory layout support.
- Adds a buffered console implementation, useful once allocation is available.
- Reworks driver/console initialization and mapping records around heap-backed
  infrastructure.
- Note: this revision's README still carries the previous "Kernel Symbols"
  title/tl;dr, so the code diff is the more reliable source for this summary.

### 19-timer-callbacks

- Branch: `19-timer-callbacks`
- Checked-out commit: `fb22c6ab`
- Extends the timer subsystem to schedule callbacks in IRQ context.
- Adds one-shot and periodic timeout support.
- Adds Raspberry Pi local interrupt controller support for timer interrupts.
- Wires timer IRQ handling into BSP exception/driver initialization.
- Demonstrates timers in `kernel_main` by scheduling delayed and periodic log
  output.

### 20-boot-improvements

- Branch: `20-boot-improvements`
- Checked-out commit: `823f8316`
- Improves early boot reporting and multicore preparation.
- Adds boot-time collection/reporting of core information from the device tree.
- Adds AArch64 LED debug support.
- Updates boot assembly and Rust boot metadata handling.
- The README is sparse and titled "Threads", but the code mainly shows boot
  and core-discovery improvements.

### 21-second-core

- Branch: `21-second-core`
- Checked-out commit: `96c8cbbc`
- Continues the multicore boot work from revision 20.
- Changes only the AArch64 boot Rust and assembly files relative to revision
  20.
- Starts refining core parking/unparking so a second core can be brought into
  kernel-controlled execution.
- The checked-out code still has the same sparse "Threads" README, so this
  appears to be an incremental work-in-progress revision.

## End-State Architecture

By `21-second-core`, the project has a workspace layout:

- `kernel/`: the main `no_std` kernel crate.
- `kernel/src/_arch/aarch64/`: architecture-specific CPU, exception, MMU, and
  time code.
- `kernel/src/bsp/raspberrypi/`: Raspberry Pi board support, memory layout,
  drivers, and exception integration.
- `kernel/src/bsp/device_driver/`: GPIO, UART, interrupt controller, and common
  device-driver code.
- `kernel/src/memory/`: MMU, mapping records, page allocation, and heap
  allocation.
- `kernel/src/exception/`: generic exception/IRQ abstractions.
- `kernel/src/time.rs`: architectural timer abstraction plus timeout callbacks.
- `libraries/`: test and debug-symbol helper crates.
- `tools/`: host-side translation-table and kernel-symbol tools.
- `xtask/`: build orchestration helper.

The final revision boots into a kernel that initializes exception handling,
memory, timers, BSP drivers, IRQs, precomputed MMU mapping records, and then
prints board/core/MMU/IRQ/heap state before entering an echo loop. It also
schedules timer callbacks, showing that IRQ-backed timer dispatch is active.

## Notes And Caveats

- The numbered submodules are pinned to specific commits; the branch names in
  `.gitmodules` describe the intended revision branch, but the worktree state is
  determined by the pinned commit.
- Several later README files are stale or mislabeled. In particular,
  `18-kernel-heap`, `20-boot-improvements`, and `21-second-core` do not have
  fully matching tutorial text. Their summaries above are based on code diffs
  and commit messages.
- The history is educational and cumulative. Some large diffs are documentation
  churn or project restructuring rather than functional changes.
