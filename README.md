# Tutorial 16 - Virtual Memory Part 4: Higher-Half Kernel

## tl;dr

- The time has come: We map and run the kernel from the top of the 64 bit virtual address space! 🥳

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Linking Changes](#linking-changes)
  - [Position-Independent Boot Code](#position-independent-boot-code)
- [Test it](#test-it)

## Introduction

A long time in the making, in this tutorial we finally map the kernel to the most significant area
(alternatively: higher-half) of the 64 bit virtual address space. This makes room for future
applications to use the whole of the least significant area of the virtual memory space.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

As has been teased since `tutorial 14`, we will make use of the `AArch64`'s `TTBR1`. Since the
kernel's virtual address space size currently is `1 GiB` (defined in
`bsp/__board_name__/memory/mmu.rs`), `TTBR1` will cover the range from `0xffff_ffff_ffff_ffff` down
to `ffff_ffff_c000_0000` (both inclusive).

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

In the `kernel.ld` linker script, we define a new symbol `__kernel_virt_start_addr` now, which is
the start address of the kernel's virtual address space, calculated as `(u64::MAX -
__kernel_virt_addr_space_size) + 1`. Before the first section definition, we set the linker script's
location counter to this address:

```ld.s
SECTIONS
{
    . =  __kernel_virt_start_addr;

    ASSERT((. & PAGE_MASK) == 0, "Start of address space is not page aligned")

    /***********************************************************************************************
    * Code + RO Data + Global Offset Table
    ***********************************************************************************************/
```

Since we are not identity mapping anymore, we start to make use of the `AT` keyword in the output
section specification:

```ld.s
/* The physical address at which the the kernel binary will be loaded by the Raspberry's firmware */
__rpi_phys_binary_load_addr = 0x80000;

/* omitted */

SECTIONS
{
    . =  __kernel_virt_start_addr;

    /* omitted */

    __code_start = .;
    .text : AT(__rpi_phys_binary_load_addr)
```

This will manifest in the kernel ELF `segment` attributes, as can be inspected using the `make
readelf` command:

```console
$ make readelf

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  LOAD           0x0000000000010000 0xffffffffc0000000 0x0000000000080000
                 0x000000000000cb08 0x000000000000cb08  R E    0x10000
  LOAD           0x0000000000020000 0xffffffffc0010000 0x0000000000090000
                 0x0000000000030dc0 0x0000000000030de0  RW     0x10000
  LOAD           0x0000000000060000 0xffffffffc0860000 0x0000000000000000
                 0x0000000000000000 0x0000000000080000  RW     0x10000

 Section to Segment mapping:
  Segment Sections...
   00     .text .rodata
   01     .data .bss
   02     .boot_core_stack

```

As you can see, `VirtAddr` and `PhysAddr` are different now, as compared to all the previous
tutorials where they were identical. This information from the `ELF` file will eventually be parsed
by the `translation table tool` and incorporated when compiling the precomputed translation tables.

You might have noticed that `.text .rodata` and `.boot_core_stack` exchanged places as compared to
previous tutorials. The reason this was done is that with a remapped kernel, this is trivial to do
without affecting the physical layout. This allows us to place an unmapped `guard page` between the
`boot core stack` and the `mmio remap region` in the VA space, which nicely protects the kernel from
stack overflows now:

```ld.s
/***********************************************************************************************
* MMIO Remap Reserved
***********************************************************************************************/
__mmio_remap_start = .;
. += 8 * 1024 * 1024;
__mmio_remap_end_exclusive = .;

ASSERT((. & PAGE_MASK) == 0, "MMIO remap reservation is not page aligned")

/***********************************************************************************************
* Guard Page
***********************************************************************************************/
. += PAGE_SIZE;

/***********************************************************************************************
* Boot Core Stack
***********************************************************************************************/
.boot_core_stack (NOLOAD) : AT(__rpi_phys_dram_start_addr)
{
    __boot_core_stack_start = .;         /*   ^             */
                                         /*   | stack       */
    . += __rpi_phys_binary_load_addr;    /*   | growth      */
                                         /*   | direction   */
    __boot_core_stack_end_exclusive = .; /*   |             */
} :segment_boot_core_stack

ASSERT((. & PAGE_MASK) == 0, "End of boot core stack is not page aligned")
```

Changes in the `_arch` `MMU` driver are minimal, and mostly concerned with configuring `TCR_EL1` for
use with `TTBR1_EL1` now. And of course, setting `TTBR1_EL1` in `fn enable_mmu_and_caching(...)`.

### Position-Independent Boot Code

Remember all the fuss that we made about `position-independent code` that will be needed until the
`MMU` is enabled. Let's quickly check what it means for us in reality now:

In `_arch/aarch64/cpu/boot.rs`, we turn on the `MMU` just before returning from `EL2` to `EL1`. So
by the time the CPU enters `EL1`, virtual memory will be active, and the CPU must therefore use the
new higher-half `virtual addresses` for everything it does.

Specifically, this means the address from which the CPU should execute upon entering `EL1` (function
`kernel_init()`) must be a valid _virtual address_, same as the stack pointer's address. Both of
them are programmed in function `fn prepare_el2_to_el1_transition(...)`, so we must ensure now that
_link-time_ addresses are used here. For this reason, retrieval of these addresses happens in
`assembly` in `boot.s`, where we can explicitly enforce generation of **absolute** addresses:

```asm
// Load the _absolute_ addresses of the following symbols. Since the kernel is linked at
// the top of the 64 bit address space, these are effectively virtual addresses.
ADR_ABS	x1, __boot_core_stack_end_exclusive
ADR_ABS	x2, kernel_init
```

Both values are forwarded to the Rust entry point function `_start_rust()`, which in turn forwards
them to `fn prepare_el2_to_el1_transition(...)`.

One more thing to consider is that we keep on programming the boot core's stack address for `EL2`
using an address that is calculated `PC-relative`, because all the `EL2` code will still run while
virtual memory _is disabled_. As such, we need the "physical" address of the stack, so to speak.

The previous tutorial also explained that it is not easily possible to compile select files using
`-fpic` in `Rust`. Still, we are doing some function calls in `Rust` before virtual memory is
enabled, so _theoretically_, there is room for failure. However, branches to local code in `AArch64`
are usually generated PC-relative. So it is a small risk worth taking. Should it still fail someday,
at least our automated CI pipeline would give notice when the tests start to fail.

## Test it

`make clippy` checks the bare-metal kernel with the selected BSP and checks the native host tools for
the build and test workflows under the host target.

That's it! We are ready for a higher-half kernel now. Power up your Raspberrys and marvel at those
beautiful (virtual) addresses:

Raspberry Pi 3:

```console
$ make chainboot
[...]

Precomputing kernel translation tables and patching kernel ELF
             ------------------------------------------------------------------------------------
                 Sections          Virt Start Addr         Phys Start Addr       Size      Attr
             ------------------------------------------------------------------------------------
  Generating .text .rodata    | 0xffff_ffff_c000_0000 | 0x0000_0000_0008_0000 |  64 KiB | C RO X
  Generating .data .bss       | 0xffff_ffff_c001_0000 | 0x0000_0000_0009_0000 | 256 KiB | C RW XN
  Generating .boot_core_stack | 0xffff_ffff_c086_0000 | 0x0000_0000_0000_0000 | 512 KiB | C RW XN
             ------------------------------------------------------------------------------------
    Patching Kernel table struct at ELF file offset 0x2_0000
    Patching Kernel tables physical base address start argument to value 0xb_0000 at ELF file offset 0x1_0088
    Finished in 0.14s

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 3

[ML] Requesting binary
[ML] Loaded! Executing the payload now

[    2.870248] mingo version 0.16.0
[    2.870456] Booting on: Raspberry Pi 3
[    2.870911] MMU online:
[    2.871203]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.872947]                         Virtual                                   Physical               Size       Attr                    Entity
[    2.874691]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.876436]       0xffff_ffff_c000_0000..0xffff_ffff_c000_ffff --> 0x00_0008_0000..0x00_0008_ffff |  64 KiB | C   RO X  | Kernel code and RO data
[    2.878050]       0xffff_ffff_c001_0000..0xffff_ffff_c004_ffff --> 0x00_0009_0000..0x00_000c_ffff | 256 KiB | C   RW XN | Kernel data and bss
[    2.879621]       0xffff_ffff_c005_0000..0xffff_ffff_c005_ffff --> 0x00_3f20_0000..0x00_3f20_ffff |  64 KiB | Dev RW XN | BCM PL011 UART
[    2.881137]                                                                                                             | BCM GPIO
[    2.882589]       0xffff_ffff_c006_0000..0xffff_ffff_c006_ffff --> 0x00_3f00_0000..0x00_3f00_ffff |  64 KiB | Dev RW XN | BCM Interrupt Controller
[    2.884214]       0xffff_ffff_c086_0000..0xffff_ffff_c08d_ffff --> 0x00_0000_0000..0x00_0007_ffff | 512 KiB | C   RW XN | Kernel boot-core stack
[    2.885818]       -------------------------------------------------------------------------------------------------------------------------------------------
```

Raspberry Pi 4:

```console
$ BSP=rpi4 make chainboot
[...]

Precomputing kernel translation tables and patching kernel ELF
             ------------------------------------------------------------------------------------
                 Sections          Virt Start Addr         Phys Start Addr       Size      Attr
             ------------------------------------------------------------------------------------
  Generating .text .rodata    | 0xffff_ffff_c000_0000 | 0x0000_0000_0008_0000 |  64 KiB | C RO X
  Generating .data .bss       | 0xffff_ffff_c001_0000 | 0x0000_0000_0009_0000 | 256 KiB | C RW XN
  Generating .boot_core_stack | 0xffff_ffff_c086_0000 | 0x0000_0000_0000_0000 | 512 KiB | C RW XN
             ------------------------------------------------------------------------------------
    Patching Kernel table struct at ELF file offset 0x2_0000
    Patching Kernel tables physical base address start argument to value 0xb_0000 at ELF file offset 0x1_0080
    Finished in 0.13s

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 4

[ML] Requesting binary
[ML] Loaded! Executing the payload now

[    2.871960] mingo version 0.16.0
[    2.871994] Booting on: Raspberry Pi 4
[    2.872449] MMU online:
[    2.872742]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.874486]                         Virtual                                   Physical               Size       Attr                    Entity
[    2.876230]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.877975]       0xffff_ffff_c000_0000..0xffff_ffff_c000_ffff --> 0x00_0008_0000..0x00_0008_ffff |  64 KiB | C   RO X  | Kernel code and RO data
[    2.879589]       0xffff_ffff_c001_0000..0xffff_ffff_c004_ffff --> 0x00_0009_0000..0x00_000c_ffff | 256 KiB | C   RW XN | Kernel data and bss
[    2.881159]       0xffff_ffff_c005_0000..0xffff_ffff_c005_ffff --> 0x00_fe20_0000..0x00_fe20_ffff |  64 KiB | Dev RW XN | BCM PL011 UART
[    2.882676]                                                                                                             | BCM GPIO
[    2.884128]       0xffff_ffff_c006_0000..0xffff_ffff_c006_ffff --> 0x00_ff84_0000..0x00_ff84_ffff |  64 KiB | Dev RW XN | GICv2 GICD
[    2.885601]                                                                                                             | GICV2 GICC
[    2.887074]       0xffff_ffff_c086_0000..0xffff_ffff_c08d_ffff --> 0x00_0000_0000..0x00_0007_ffff | 512 KiB | C   RW XN | Kernel boot-core stack
[    2.888678]       -------------------------------------------------------------------------------------------------------------------------------------------
```
