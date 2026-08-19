# Tutorial 18 - Kernel Heap

## tl;dr

- A global kernel heap allocator is added.
- The heap is reserved in the kernel linker script and initialized during early kernel setup.
- Kernel code can now use selected `alloc` types, which is used immediately to simplify driver and
  interrupt-handler bookkeeping.

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Heap Memory Region](#heap-memory-region)
  - [Global Allocator](#global-allocator)
  - [Using Allocation In Kernel Code](#using-allocation-in-kernel-code)
- [Test it](#test-it)

## Introduction

Previous tutorials avoided dynamic allocation. That made the kernel easier to reason about while
the boot path, MMU, exception handling, interrupts, and symbol lookup were still being built.
However, fixed-size arrays are awkward for subsystems whose size is naturally discovered at runtime,
such as registered drivers and IRQ handlers.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

This tutorial adds a kernel heap. The heap is still deliberately small and controlled: it is carved
out by the linker, mapped by the existing MMU setup, and initialized once during kernel startup.
After that, the kernel can use `alloc` collections where they remove artificial fixed limits.

## Implementation

### Heap Memory Region

The Raspberry Pi linker script gains a dedicated `.heap` output section after `.data`/`.bss` and
before the virtual MMIO remap reservation. The section is `NOLOAD`, so it reserves address space
without increasing the kernel image with zero-filled bytes. The current heap size is `16 MiB`.

The BSP memory module exposes the linker-provided heap start and end symbols, and the BSP MMU code
adds a virtual heap region that is mapped with normal cacheable memory attributes.

### Global Allocator

`kernel/src/memory/heap_alloc.rs` defines the kernel heap allocator. It wraps
`linked_list_allocator::Heap` in the existing IRQ-safe lock type and installs it as the Rust global
allocator with `#[global_allocator]`.

The allocator is initialized from `memory::init()`, after the MMIO virtual-address allocator is
prepared. This keeps all memory-subsystem initialization in one place:

- the MMIO allocator manages virtual pages for device registers;
- the heap allocator manages normal kernel memory for `alloc` users.

The allocator also exposes `print_usage()`, which is called from `kernel_main()` so boot logs show
used and free heap space.

### Using Allocation In Kernel Code

The kernel binary enables `extern crate alloc`, and `linked_list_allocator` is added to the kernel
crate dependencies.

The driver manager is one of the first users. Its descriptor storage changes from a fixed-size
array to a `Vec<DeviceDriverDescriptor<_>>`. This removes the previous hardcoded driver-count limit
and lets driver registration scale with the BSP's actual device list.

IRQ handler tables in the interrupt-controller code are also prepared to use heap-backed `Vec`
storage where appropriate.

## Test it

Build and boot the tutorial as before. In the normal boot log, the kernel now prints a `Kernel heap`
section with current usage information before entering the echo loop.
