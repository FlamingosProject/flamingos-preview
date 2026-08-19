# Tutorial 18 - Kernel Heap

## tl;dr

- A global kernel heap allocator is added.
- The heap is reserved in the kernel linker script and initialized during early kernel setup.
- Kernel code can now use selected `alloc` types, which is used immediately to simplify driver and
  interrupt-handler bookkeeping.

## Table of Contents

- [Introduction](#introduction)
- [Build and boot](#build-and-boot)
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

This tutorial adds a kernel heap. The heap is still deliberately small and controlled: it is carved
out by the linker, mapped by the existing MMU setup, and initialized once during kernel startup.
After that, the kernel can use `alloc` collections where they remove artificial fixed limits.

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

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

Build and boot the tutorial as before. In the normal boot log, the kernel now prints a `Kernel heap`
section with current usage information before entering the echo loop.

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
