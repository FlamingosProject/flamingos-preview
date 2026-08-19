# Tutorial 16 - Virtual Memory Part 4: Higher-Half Kernel

## tl;dr

- The time has come: We map and run the kernel from the top of the 64 bit virtual address space! 🥳

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Linking Changes](#linking-changes)
  - [Booting into the higher half](#booting-into-the-higher-half)
- [Build and boot](#build-and-boot)
- [Test it](#test-it)

## Introduction

A long time in the making, in this tutorial we finally map the kernel to the most significant area
(alternatively: higher-half) of the 64 bit virtual address space. This makes room for future
applications to use the whole of the least significant area of the virtual memory space.

As has been teased since `tutorial 14`, we will make use of the `AArch64`'s `TTBR1`. Since the
kernel's virtual address space size currently is `1 GiB` (defined in
`bsp/__board_name__/memory/mmu.rs`), `TTBR1` will cover the range from `0xffff_ffff_ffff_ffff` down
to `0xffff_ffff_c000_0000` (both inclusive).

## Implementation

In `src/memory/mmu.rs`, we extend the `AssociatedTranslationTable` trait with a `TableStartFromTop`
associated type:

```rust
/// Intended to be implemented for [`AddressSpace`].
pub trait AssociatedTranslationTable {
    /// A translation table whose address range is:
    ///
    /// [u64::MAX, (u64::MAX - AS_SIZE) + 1]
    type TableStartFromTop;

    /// A translation table whose address range is:
    ///
    /// [AS_SIZE - 1, 0]
    type TableStartFromBottom;
}
```

Architecture specific code in `_arch/aarch64/memory/mmu/translation_table.rs` populates both types
now by making use of a new generic that is added to `FixedSizeTranslationTable`, which defines
whether the covered address space starts from the top or the bottom:

```rust
pub struct FixedSizeTranslationTable<const NUM_TABLES: usize, const START_FROM_TOP: bool> {
    ...
```

```rust
impl<const AS_SIZE: usize> memory::mmu::AssociatedTranslationTable
    for memory::mmu::AddressSpace<AS_SIZE>
where
    [u8; Self::SIZE >> Granule512MiB::SHIFT]: Sized,
{
    type TableStartFromTop =
        FixedSizeTranslationTable<{ Self::SIZE >> Granule512MiB::SHIFT }, true>;

    type TableStartFromBottom =
        FixedSizeTranslationTable<{ Self::SIZE >> Granule512MiB::SHIFT }, false>;
}
```

Thanks to this infrastructure, `BSP` Rust code in `bsp/__board_name__/memory/mmu.rs` only needs to
change to this newly introduced type in order to switch from lower half to higher half translation
tables for the kernel:

```rust
type KernelTranslationTable =
    <KernelVirtAddrSpace as AssociatedTranslationTable>::TableStartFromTop;
```

### Linking Changes

`kernel/src/bsp/raspberrypi/kernel.ld` sets the location counter
to the start of the higher-half kernel address space. The
`AT(...)` expressions set the physical load addresses. For
example, `.text` starts at virtual `0xffff_ffff_c000_0000` and
loads at physical `0x80000`. Run `make readelf` to compare
`VirtAddr` and `PhysAddr` for the ELF loadable segments.

The ELF loader copies each segment to its physical address.
The bootstrap stack still occupies physical addresses from
64 KiB to 512 KiB. Its virtual mapping moves above the MMIO
reservation, with a guard page between them. This lets a stack
overflow fault before reaching the MMIO range.

### Booting into the higher half

Stage 1 loads the ELF and transfers control to stage 2 at the
physical bootstrap address with the MMU off. Stage 2 uses a
physical stack and reads the linker-provided virtual bounds and
physical bases to populate the tables. The architecture code
configures `TCR_EL1` for the top-of-address-space mapping and
installs the table base in `TTBR1_EL1`.

The assembly loads absolute linked addresses for the EL1
entry and stack. Once translation is enabled, `eret` takes
execution to the high virtual entry and the kernel uses its
high virtual stack. The low virtual range is available for a
future application address space.

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

## Test it

```sh
make test BSP=rpi3
make readelf BSP=rpi3
```

Check that the first executable loadable segment reports the
linked higher-half address and the physical load address
`0x80000`. When the kernel boots, the `MMU online` report lists
its high virtual mappings alongside the low physical ranges.

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
