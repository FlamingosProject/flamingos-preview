# Tutorial 20 - Boot Improvements

## tl;dr

- Early boot now parses the device tree pointer that has been preserved since Chapter 02.
- The kernel records CPU core IDs from the device tree and prints them during startup.
- The existing raw-GPIO LED trace remains available across the expanded boot path.

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Device Tree Parsing](#device-tree-parsing)
  - [Core Discovery](#core-discovery)
  - [Boot Trace Coverage](#boot-trace-coverage)
- [Test it](#test-it)

## Introduction

The previous tutorial made timer IRQ callbacks work, which is one of the pieces needed before
moving toward multicore and scheduling work. This revision tightens the earliest boot path so later
code can reason about the machine it is running on.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

The assembly-to-Rust handoff has preserved the firmware-provided device tree pointer since Chapter
02. The key change here is that Rust finally interprets it, storing basic core information for later
reporting.

## Implementation

### Device Tree Parsing

The existing boot arguments already carry the incoming device tree pointer through assembly. Early
boot converts it to an address in the mapped boot-stack region and preserves it across the MMU
transition. The kernel crate now adds the `fdt` dependency and parses the flattened device tree once
linked virtual code is safe to execute.

QEMU's `raspi3b` machine supplies the older `ATAG_CORE` handoff instead of a flattened device tree.
The boot code recognizes that specific format and records the four cores provided by the supported
Raspberry Pi BSPs, preserving the existing QEMU development and test workflows.

### Core Discovery

The AArch64 boot module adds `CoresInfo`, a small static record containing the number of discovered
cores and their IDs. At the start of `kernel_init()`, `process_device_tree()` walks the device tree
CPU nodes and fills this structure.

`kernel_main()` prints the discovered core IDs as part of the normal boot log. This makes the
firmware handoff visible and gives later multicore work a concrete source of topology data.

### Boot Trace Coverage

The raw GPIO-based LED helpers for Raspberry Pi 3 and Raspberry Pi 4 have existed since Chapter 02.
This chapter carries them through the expanded boot path. Stage 3 marks the last physical-address
phase before the MMU transition; device-tree parsing is deliberately deferred until linked virtual
code is safe to execute.

The tracing is controlled by the `boot_trace` feature. When the feature is disabled, the blink
macros compile to no-op control flow.

## Test it

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

Boot the kernel as before. The startup log now includes a `Cores:` section showing the core IDs
parsed from the device tree. To diagnose very early boot on hardware, use the Chapter 15 argument
forwarding with `cargo xtask build rpi3 --features=boot_trace` and observe the board LED blink codes.
