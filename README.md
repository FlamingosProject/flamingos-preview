# Tutorial 19 - Timer Callbacks

## tl;dr

- The timer subsystem can now execute one-shot and periodic timeout callbacks.
- The AArch64 architectural timer is programmed to raise IRQs instead of only being used for polling
  and timestamp reads.
- The Raspberry Pi local interrupt controller is added so timer IRQs can be routed and handled.

## Table of Contents

- [Introduction](#introduction)
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

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

The feature is intentionally small. It provides the mechanism needed by later scheduling and
threading work without introducing a full scheduler yet.

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

Boot the kernel and watch the UART log. After the usual startup diagnostics, the kernel schedules
callbacks that print after roughly two seconds, five seconds, and then once per second for the
periodic timer.
