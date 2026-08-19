# Tutorial 12 - Integrated Testing

## Introduction

Kernel code cannot use the host platform's normal test harness. The code under
test runs without an operating system, uses the kernel's own entry point and
panic handler, and must be booted as an AArch64 image.

This chapter introduces a stable-Rust testing path built from ordinary Cargo
integration-test targets:

- each integration test is a small, self-contained kernel;
- Cargo builds each test without its standard host test harness;
- a Rust target runner converts the test ELF to a boot image and starts QEMU;
- the test kernel reports success or failure through QEMU's semihosting exit
  mechanism; and
- the host runner fails tests that do not terminate within a fixed timeout.

No compiler-generated custom test framework is required. Test execution and
ordering remain explicit in the test kernel, which keeps the mechanism small
and suitable for a bare-metal tutorial.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

## Workspace Layout

The repository is now a Cargo workspace:

```text
.
├── Cargo.toml
├── kernel
│   ├── Cargo.toml
│   ├── src
│   └── tests
└── tools
    └── kernel_test_runner
```

The `kernel` package provides both the normal kernel binary and the
`libkernel` library used by integration-test kernels. The
`kernel_test_runner` package is an ordinary host program.

## Harness-Free Test Kernels

Cargo normally creates a host-oriented test harness for files under
`kernel/tests`. A bare-metal test supplies its own entry point instead, so its
manifest entry disables that harness:

```toml
[[test]]
name = "01_timer_sanity"
harness = false
```

The test is a normal `no_std`, `no_main` kernel:

```rust
#![no_main]
#![no_std]

use libkernel::{bsp, cpu, exception, memory, time};

#[no_mangle]
unsafe fn kernel_init() -> ! {
    exception::handling_init();
    memory::init();
    bsp::driver::qemu_bring_up_console();

    assert!(time::time_manager().uptime().as_nanos() > 0);

    cpu::qemu_exit_success()
}
```

Assertions use the kernel panic handler. A panic in a `test_build` exits QEMU
with failure, while reaching `qemu_exit_success()` makes the host runner
succeed.

Tests containing several checks call them in a visible, deterministic order
from `kernel_init()`. This avoids hidden registration machinery and makes the
test's boot requirements easy to inspect.

## Expected Panics

Some architectural tests succeed only if the kernel takes an exception. The
synchronous page-fault test is one example.

Immediately before performing the faulting operation, it marks the next panic
as expected:

```rust
test::expect_panic();
core::ptr::read_volatile((9 * 1024 * 1024 * 1024) as *const u64);
```

The test-build panic path consumes that marker and exits QEMU successfully. A
panic before the marker remains a failure, and reaching the instruction after
the faulting access explicitly exits with failure.

## Host Test Runner

Cargo supports a target runner for executables that cannot run directly on
the host. The Makefile builds `tools/kernel_test_runner` for the host and
provides its path through:

```text
CARGO_TARGET_AARCH64_UNKNOWN_NONE_SOFTFLOAT_RUNNER
```

For each test ELF, the runner:

1. creates a stripped raw image with `rust-objcopy`;
2. starts the board's QEMU machine with semihosting enabled;
3. inherits QEMU's console output for diagnostics;
4. waits for QEMU's success or failure status; and
5. terminates and fails a test that exceeds its timeout.

The runner is intentionally generic. Test-specific behavior belongs in the
test kernel, and interactive console scripting is outside this test suite.

## Included Tests

### Boot Smoke Test

`make test_boot` builds the normal kernel with `test_build`. The kernel exits
successfully only after completing its normal initialization path. This
catches failures that happen before the main kernel loop.

### Timer Sanity

`01_timer_sanity` verifies that:

- the architectural timer is advancing;
- its reported resolution is nonzero and below 100 nanoseconds; and
- a one-second spin delay advances the timer by one second.

### Synchronous Page Fault

`02_exception_sync_page_fault` reads from an unmapped address and succeeds
only when the resulting synchronous exception reaches the expected panic
path.

## Commands

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

Run the boot smoke test:

```console
make test_boot
```

Run all integration-test kernels:

```console
make test_integration
```

Run one integration-test kernel:

```console
TEST=01_timer_sanity make test_integration
```

Run the complete Chapter 12 test set:

```console
make test
```

The Raspberry Pi 4 does not have a QEMU machine configured in this tutorial,
so QEMU-backed tests report that they are unavailable when `BSP=rpi4` is
selected.

## Adding A Test

1. Add a `no_std`, `no_main` file under `kernel/tests`.
2. Register it as a `[[test]]` with `harness = false` in
   `kernel/Cargo.toml`.
3. Initialize only the kernel subsystems required by the test.
4. Use assertions for failure and call `cpu::qemu_exit_success()` after all
   checks pass.
5. Ensure every failure path either panics or calls
   `cpu::qemu_exit_failure()`.

Prefer a small number of architectural integration tests with clear value.
Tests requiring elaborate host interaction should justify that additional
infrastructure before being added.
