# Tutorial 21 - Second Core

## tl;dr

- Secondary cores reaching stage 2 have an explicit release hook.
- Early boot introduces a parking flag array that can be used to release secondary cores later.
- A placeholder secondary-core entry point is added so the next stage of multicore bring-up has a
  concrete target.

## Table of Contents

- [Introduction](#introduction)
- [Build and boot](#build-and-boot)
- [Implementation](#implementation)
  - [Core Parking State](#core-parking-state)
  - [Secondary-Core Entry Point](#secondary-core-entry-point)
- [Test it](#test-it)

## Introduction

This revision is a small, focused step toward running code on a second CPU core. The previous boot path parked non-boot cores permanently if they entered the kernel.
This chapter adds a release hook for those cores. Releasing firmware-parked cores remains future work.

The code now distinguishes the boot core from secondary cores and leaves the secondary cores parked
until they are explicitly released.

## Build and boot

From the chapter root, run `cargo xtask build rpi3` or `make`.
Use `rpiz2` for Zero 2 W and `rpi4` for Pi 4. A normal build
produces two files:

- `kernel.elf` is the UART payload and the input to the ELF
  loader. It retains symbol and string tables and unwind data.
- `kernel8.img` is the flat firmware image with the same ELF
  embedded in its bootstrap. Use it for standalone SD boot or
  QEMU.

For serial boot, build `chainloader8.img` with
`cargo xtask build rpi3 --features chainloader` or
`CHAINLOADER=1 make BSP=rpi3`. Copy it to the FAT boot partition
under the firmware's kernel filename, normally `kernel8.img`.
Then run `make chainboot BSP=rpi3` or pass `kernel.elf` to
`scip --binfile` using the same serial settings as before.
Install the ELF-capable receiver before uploading the ELF.

`cargo xtask package rpi3 <kernel-elf> <output-image>` embeds an
existing kernel ELF in a firmware image. QEMU integration tests
package their own kernel ELFs this way.

For JTAG, `make jtagboot` wraps the Chapter 08 parking helper in
an ELF for serial upload. With OpenOCD connected, `make gdb`
and `make gdb-opt0` load a packaged bootstrap at physical
`0x80000` and add stage-2 symbols at their linked virtual
addresses. This workflow awaits hardware validation.

## Implementation

### Core Parking State

The AArch64 boot module adds a `BOOT_PARK` static array of `AtomicBool` values. Its size is tied to
`MAX_CORES`, which is reduced from `64` to `16` for this early implementation.

When assembly boot code detects that the current CPU is not the boot core, it enters a `wfe` loop.
After each wake event, it checks the corresponding parking flag. While the flag is zero, the core
continues waiting. Once the flag is set, the core leaves the parking loop.

### Secondary-Core Entry Point

The Rust boot module adds `_start_core(id)`, an exported placeholder entry point for a released
secondary core. In this revision it still reports progress through the early panic/blink-code path,
which makes it clear that the secondary-core path has not yet become a full kernel thread or
scheduler entry.

The assembly path branches to `_start_core` after a secondary core is released from the parking
loop.

## Test it

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

For normal single-core boot behavior, this revision should behave like the previous one. The
secondary-core path is preparatory: it introduces the parking/release structure, but higher-level
code to set the release flag and start useful work on the second core is still to come.

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
