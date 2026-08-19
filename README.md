# Tutorial 19 - Timer Callbacks

## tl;dr

- The timer subsystem can now execute one-shot and periodic timeout callbacks.
- The AArch64 architectural timer is programmed to raise IRQs instead of only being used for polling
  and timestamp reads.
- The Raspberry Pi local interrupt controller is added so timer IRQs can be routed and handled.

## Table of Contents

- [Introduction](#introduction)
- [Build and boot](#build-and-boot)
- [Implementation](#implementation)
  - [Timer IRQ Programming](#timer-irq-programming)
  - [Local Interrupt Controller](#local-interrupt-controller)
  - [Timeout Management](#timeout-management)
- [Test it](#test-it)

## Introduction

Earlier tutorials used the architectural timer for timestamps and spin delays. This revision turns
the timer into an asynchronous kernel facility: code can register callbacks that should run after a
duration, and the timer subsystem arranges for the next due callback to be delivered from IRQ
context.

The feature is intentionally small. It provides the mechanism needed by later scheduling and
threading work without introducing a full scheduler yet.

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

### Timer IRQ Programming

The AArch64 timer module gains helpers to:

- report the IRQ number used by the non-secure physical timer;
- program `CNTP_CVAL_EL0` for a target due time;
- enable the timer interrupt via `CNTP_CTL_EL0`;
- conclude a pending timeout IRQ by disabling the timer.

This moves the subsystem beyond `uptime()` and `spin_for()` and lets it request an interrupt at a
specific future time.

### Local Interrupt Controller

Raspberry Pi timer IRQs are delivered through local core interrupt-controller state, not only
through the peripheral interrupt controller used for UART IRQs. This revision adds a Broadcom local
interrupt-controller driver under the existing BCM interrupt-controller module.

The local controller stores IRQ handler descriptors, enables timer IRQ bits for core 0, and reports
pending local IRQs to the generic IRQ manager path.

### Timeout Management

`kernel/src/time.rs` grows a timeout manager on top of the architectural timer. It supports:

- one-shot callbacks;
- periodic callbacks;
- scheduling the next hardware timer event from the earliest due callback;
- executing callbacks when the timer IRQ fires.

The kernel initializes the timer subsystem during `kernel_init()` before driver IRQ setup is
completed. `kernel_main()` demonstrates the feature by registering two one-shot callbacks and one
periodic callback.

## Test it

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

Boot the kernel and watch the UART log. After the usual startup diagnostics, the kernel schedules
callbacks that print after roughly two seconds, five seconds, and then once per second for the
periodic timer.

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
