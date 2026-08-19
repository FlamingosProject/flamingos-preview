# Tutorial 21 - Second Core

## tl;dr

- Non-boot cores are no longer treated as immediate boot failures.
- Early boot introduces a parking flag array that can be used to release secondary cores later.
- A placeholder secondary-core entry point is added so the next stage of multicore bring-up has a
  concrete target.

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Core Parking State](#core-parking-state)
  - [Secondary-Core Entry Point](#secondary-core-entry-point)
- [Test it](#test-it)

## Introduction

This revision is a small, focused step toward running code on a second CPU core. The previous boot
path identified non-boot cores and treated their presence in the entry path as a panic condition.
That is useful while the kernel is single-core only, but it is not enough for controlled multicore
bring-up.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

The code now distinguishes the boot core from secondary cores and leaves the secondary cores parked
until they are explicitly released.

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

For normal single-core boot behavior, this revision should behave like the previous one. The
secondary-core path is preparatory: it introduces the parking/release structure, but higher-level
code to set the release flag and start useful work on the second core is still to come.
