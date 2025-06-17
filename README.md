# Tutorial 09 - Privilege Level

## tl;dr

- In early boot code, we transition from the `Hypervisor` privilege level (`EL2` in AArch64)
  to the `Kernel` (`EL1`) privilege level.

## Table of Contents

- [Tutorial 09 - Privilege Level](#tutorial-09---privilege-level)
  - [tl;dr](#tldr)
  - [Table of Contents](#table-of-contents)
  - [Introduction](#introduction)
  - [Scope of this tutorial](#scope-of-this-tutorial)
  - [Checking for EL2 in the entrypoint](#checking-for-el2-in-the-entrypoint)
  - [Transition preparation](#transition-preparation)
  - [Returning from an exception that never happened](#returning-from-an-exception-that-never-happened)
  - [Test it](#test-it)

## Introduction

Application-grade CPUs have so-called `privilege levels`, which have different purposes:

| Typically used for     | AArch64 | RISC-V | x86     |
| ---------------------- | ------- | ------ | ------- |
| Userspace applications | EL0     | U/VU   | Ring 3  |
| OS Kernel              | EL1     | S/VS   | Ring 0  |
| Hypervisor             | EL2     | HS     | Ring -1 |
| Low-Level Firmware     | EL3     | M      |         |

`EL` in AArch64 stands for `Exception Level`. If you want more information regarding the
other architectures, please have a look at the following links:
- [x86 privilege rings].
- [RISC-V privilege modes].

At this point, I strongly recommend that you take a look at the [AArch64 Exception Model]
before you continue. You might also want to take a look at the (massive) [Programmer's Guide
for ARMv8-A].

[x86 privilege rings]: https://en.wikipedia.org/wiki/Protection_ring
[RISC-V privilege modes]: https://five-embeddev.com/riscv-priv-isa-manual/Priv-v1.12/priv-intro.html#privilege-levels
[AArch64 Exception Model]: https://developer.arm.com/documentation/102412/0103/Privilege-and-Exception-levels/Types-of-privilege
[Programmer’s Guide for ARMv8-A]: https://developer.arm.com/documentation/ddi0487/lb

## Scope of this tutorial

The Raspberry Pi pre-boot firmware will start in `EL3` and give us control at `EL2`. It is
more normal to run OS kernel code at `EL1`, so we will transition there.

## Checking for EL2 in the entrypoint

First of all, we need to ensure that we are actually executing in `EL2` before we can call
respective code to transition to `EL1`. Therefore, we add a new check to the top of
`boot.s`, which parks the CPU core should it not be in `EL2`.

```
// Only proceed if the core executes in EL2. Park it otherwise.
mrs	x0, CurrentEL
cmp	x0, {CONST_CURRENTEL_EL2}
b.ne	.L_parking_loop
```

Afterwards, we continue with preparing the `EL2` -> `EL1` transition by calling
`prepare_el2_to_el1_transition()` in `boot.rs`:

```rust
#[no_mangle]
pub unsafe extern "C" fn _start_rust(phys_boot_core_stack_end_exclusive_addr: u64) -> ! {
    prepare_el2_to_el1_transition(phys_boot_core_stack_end_exclusive_addr);

    // Use `eret` to "return" to EL1. This results in execution of kernel_init() in EL1.
    asm::eret()
}
```

## Transition preparation

Since `EL2` is more privileged than `EL1`, it has control over various processor features and can
allow or disallow `EL1` code to use them. One such example is access to timer and counter registers.
We are already using them since [tutorial 07](../07_timestamps/), so of course we want to keep them.
Therefore we set the respective flags in the [Counter-timer Hypervisor Control register] and
additionally set the virtual offset to zero so that we get the real physical value everytime.
(Note that `tock_register::fields::FieldValues` currently must be combined with `+` rather than `|`:
see the [`BitOr` issue].)

[`BitOr`issue]: https://github.com/tock/tock/issues/4469

[Counter-timer Hypervisor Control register]:  https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/cnthctl_el2.rs.html

```rust
// Enable timer counter registers for EL1.
CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);

// No offset for reading the counters.
CNTVOFF_EL2.set(0);
```

Next, we configure the [Hypervisor Configuration Register] such that `EL1` runs in `AArch64` mode,
and not in `AArch32`, which would also be possible.

[Hypervisor Configuration Register]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/hcr_el2.rs.html

```rust
// Set EL1 execution state to AArch64.
HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);
```

## Returning from an exception that never happened

There is actually only one way to transition from a higher EL to a lower EL, which is by way of
executing the [ERET] instruction.

[ERET]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/asm.rs.html#92-101

This instruction will copy the contents of the [Saved Program Status Register - EL2] to `Current
Program Status Register - EL1` and jump to the instruction address that is stored in the [Exception
Link Register - EL2].

This is basically the reverse of what is happening when an exception is taken. You'll learn about
that in an upcoming tutorial.

[Saved Program Status Register - EL2]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/spsr_el2.rs.html
[Exception Link Register - EL2]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/elr_el2.rs.html

```rust
// Set up a simulated exception return.
//
// First, fake a saved program status where all interrupts were masked and SP_EL1 was used as a
// stack pointer.
SPSR_EL2.write(
    SPSR_EL2::D::Masked
        + SPSR_EL2::A::Masked
        + SPSR_EL2::I::Masked
        + SPSR_EL2::F::Masked
        + SPSR_EL2::M::EL1h,
);

// Second, let the link register point to kernel_init().
ELR_EL2.set(crate::kernel_init as *const () as u64);

// Set up SP_EL1 (stack pointer), which will be used by EL1 once we "return" to it. Since there
// are no plans to ever return to EL2, just re-use the same stack.
SP_EL1.set(phys_boot_core_stack_end_exclusive_addr);
```

As you can see, we are populating `ELR_EL2` with the address of the `kernel_init()` function that we
earlier used to call directly from the entrypoint. Finally, we set the stack pointer for `SP_EL1`.

You might have noticed that the stack's address was supplied as a function argument. As you might
remember, in  `_start()` in `boot.s`, we are already setting up the stack for `EL2`. Since there
are no plans to ever return to `EL2`, we can just re-use the same stack for `EL1`, so its address is
forwarded using function arguments.

Lastly, back in `_start_rust()` a call to `ERET` is made:

```rust
#[no_mangle]
pub unsafe extern "C" fn _start_rust(phys_boot_core_stack_end_exclusive_addr: u64) -> ! {
    prepare_el2_to_el1_transition(phys_boot_core_stack_end_exclusive_addr);

    // Use `eret` to "return" to EL1. This results in execution of kernel_init() in EL1.
    asm::eret()
}
```

## Test it

In `main.rs`, we print the `current privilege level` and additionally inspect if the mask bits in
`SPSR_EL2` made it to `EL1` as well:

```console
$ make chainboot
[...]
Minipush 1.0

[MP] ⏳ Waiting for /dev/ttyUSB0
[MP] ✅ Serial connected
[MP] 🔌 Please power the target now

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 3

[ML] Requesting binary
[MP] ⏩ Pushing 14 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.162546] mingo version 0.9.0
[    0.162745] Booting on: Raspberry Pi 3
[    0.163201] Current privilege level: EL1
[    0.163677] Exception handling state:
[    0.164122]       Debug:  Masked
[    0.164511]       SError: Masked
[    0.164901]       IRQ:    Masked
[    0.165291]       FIQ:    Masked
[    0.165681] Architectural timer resolution: 52 ns
[    0.166255] Drivers loaded:
[    0.166592]       1. BCM PL011 UART
[    0.167014]       2. BCM GPIO
[    0.167371] Timer test, spinning for 1 second
[    1.167904] Echoing input now
```

