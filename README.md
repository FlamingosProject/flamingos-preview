# Tutorial 10 - Virtual Memory Part 1: Identity Map All The Things!

## tl;dr

- The `MMU` is turned on.
- A simple scheme is used: Static `64 KiB` translation tables.
- For educational purposes, we write to a remapped `UART`, and `identity map` everything else.

## Table of Contents

- [Tutorial 10 - Virtual Memory Part 1: Identity Map All The Things!](#tutorial-10---virtual-memory-part-1-identity-map-all-the-things)
  - [tl;dr](#tldr)
  - [Table of Contents](#table-of-contents)
  - [Introduction](#introduction)
  - [MMU and paging theory](#mmu-and-paging-theory)
  - [Approach](#approach)
    - [Generic Kernel code: `memory/mmu.rs`](#generic-kernel-code-memorymmurs)
    - [BSP: `bsp/raspberrypi/memory/mmu.rs`](#bsp-bspraspberrypimemorymmurs)
    - [AArch64: `_arch/aarch64/memory/*`](#aarch64-_archaarch64memory)
    - [`kernel.ld`](#kernelld)
  - [Address translation examples](#address-translation-examples)
    - [Address translation using a 64 KiB page descriptor](#address-translation-using-a-64-kib-page-descriptor)
  - [Zero-cost abstraction](#zero-cost-abstraction)
  - [Test it](#test-it)

## Introduction

Virtual memory is an immensely complex, but important and powerful topic. In this tutorial, we start
slow and easy by switching on the `MMU`, using static translation tables and `identity-map`
everything at once (except for the `UART`, which we also remap a second time for educational
purposes; This will be gone again in the next tutorial).

## MMU and paging theory

At this point, we will not re-invent the wheel and go into detailed descriptions of how paging in
modern application-grade processors works. The internet is full of great resources regarding this
topic, and we encourage you to read some of it to get a high-level understanding of the topic.

To follow the rest of this `AArch64` specific tutorial, I strongly recommend that you stop right
here and first read `Chapter 12` of the [ARM Cortex-A Series Programmer's Guide for ARMv8-A] before
you continue. This will set you up with all the `AArch64`-specific knowledge needed to follow along.

Back from reading `Chapter 12` already? Good job :+1:!

[ARM Cortex-A Series Programmer's Guide for ARMv8-A]: http://infocenter.arm.com/help/topic/com.arm.doc.den0024a/DEN0024A_v8_architecture_PG.pdf

## Approach

1. The generic `kernel` part: `src/memory/mmu.rs` and its submodules provide architecture-agnostic
   descriptor types for composing a high-level data structure that describes the kernel's virtual
   memory layout: `memory::mmu::KernelVirtualLayout`.
2. The `BSP` part: `src/bsp/raspberrypi/memory/mmu.rs` contains a static instance of
   `KernelVirtualLayout` and makes it accessible through the function
   `bsp::memory::mmu::virt_mem_layout()`.
3. The `aarch64` part: `src/_arch/aarch64/memory/mmu.rs` and its submodules contain the actual `MMU`
   driver. It picks up the `BSP`'s high-level `KernelVirtualLayout` and maps it using a `64 KiB`
   granule.

### Generic Kernel code: `memory/mmu.rs`

The descriptor types provided in this file are building blocks which help to describe attributes of
different memory regions. For example, `R/W`, `no-execute`, `cached/uncached`, and so on.

The descriptors are agnostic of the hardware `MMU`'s actual descriptors. Different `BSP`s can use
these types to produce a high-level description of the kernel's virtual memory layout. The actual
`MMU` driver for the real HW will consume these types as an input.

This way, we achieve a clean abstraction between `BSP` and `_arch` code, which allows exchanging one
without needing to adapt the other.

### BSP: `bsp/raspberrypi/memory/mmu.rs`

This file contains an instance of `KernelVirtualLayout`, which stores the descriptors mentioned
previously. The `BSP` is the correct place to do this, because it has knowledge of the target
board's memory map.

The policy is to only describe regions that are **not** ordinary, normal cacheable DRAM. However,
nothing prevents you from defining those too if you wish to. Here is an example for the device MMIO
region:

```rust
TranslationDescriptor {
    name: "Device MMIO",
    virtual_range: mmio_range_inclusive,
    physical_range_translation: Translation::Identity,
    attribute_fields: AttributeFields {
        mem_attributes: MemAttributes::Device,
        acc_perms: AccessPermissions::ReadWrite,
        execute_never: true,
    },
},
```

`KernelVirtualLayout` itself implements the following method:

```rust
pub fn virt_addr_properties(
    &self,
    virt_addr: usize,
) -> Result<(usize, AttributeFields), &'static str>
```

It will be used by `_arch/aarch64`'s `MMU` code to request attributes for a virtual address and the
translation, which delivers the physical output address (the `usize` in the return-tuple). The
function scans for a descriptor that contains the queried address, and returns the respective
findings for the first entry that is a hit. If no entry is found, it returns default attributes for
normal cacheable DRAM and the input address, hence telling the `MMU` code that the requested
address should be `identity mapped`.

Due to this default behavior, it is not needed to define normal cacheable DRAM regions.

### AArch64: `_arch/aarch64/memory/*`

These modules contain the `AArch64` `MMU` driver. The granule is hardcoded here (`64 KiB` page
descriptors).

In `translation_table.rs`, there is a definition of the actual translation table struct which is
generic over the number of `LVL2` tables. The latter depends on the size of the target board's
memory. Naturally, the `BSP` knows these details about the target board, and provides the size
through the constant `bsp::memory::mmu::KernelAddrSpace::SIZE`.

This information is used by `translation_table.rs` to calculate the number of needed `LVL2` tables.
Since one `LVL2` table in a `64 KiB` configuration covers `512 MiB`, all that needs to be done is to
divide `KernelAddrSpace::SIZE` by `512 MiB` (there are several compile-time checks in place that
ensure that `KernelAddrSpace::SIZE` is a multiple of `512 MiB`).

The final table type is exported as `KernelTranslationTable`. Below is the respective excerpt from
`translation_table.rs`:

```rust
/// A table descriptor for 64 KiB aperture.
///
/// The output points to the next table.
#[derive(Copy, Clone)]
#[repr(C)]
struct TableDescriptor {
    value: u64,
}

/// A page descriptor with 64 KiB aperture.
///
/// The output points to physical memory.
#[derive(Copy, Clone)]
#[repr(C)]
struct PageDescriptor {
    value: u64,
}

const NUM_LVL2_TABLES: usize = bsp::memory::mmu::KernelAddrSpace::SIZE >> Granule512MiB::SHIFT;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Big monolithic struct for storing the translation tables. Individual levels must be 64 KiB
/// aligned, hence the "reverse" order of appearance.
#[repr(C)]
#[repr(align(65536))]
pub struct FixedSizeTranslationTable<const NUM_TABLES: usize> {
    /// Page descriptors, covering 64 KiB windows per entry.
    lvl3: [[PageDescriptor; 8192]; NUM_TABLES],

    /// Table descriptors, covering 512 MiB windows.
    lvl2: [TableDescriptor; NUM_TABLES],
}

/// A translation table type for the kernel space.
pub type KernelTranslationTable = FixedSizeTranslationTable<NUM_LVL2_TABLES>;
```

In `mmu.rs`, `KernelTranslationTable` is then used to create the final instance of the kernel's
tables:

```rust
//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

/// The kernel translation tables.
static mut KERNEL_TABLES: KernelTranslationTable = KernelTranslationTable::new();
```

They are populated during `MMU::init()` by calling `KERNEL_TABLES.populate_tt_entries()`, which
utilizes `bsp::memory::mmu::virt_mem_layout().virt_addr_properties()` and a bunch of utility
functions that convert the kernel generic descriptors to the actual `64 bit` integer entries needed
by the `AArch64 MMU` hardware for the translation table arrays.

One notable thing is that each page descriptor has an entry (`AttrIndex`) that indexes into the
[MAIR_EL1] register, which holds information about the cacheability of the respective page. We
currently define normal cacheable memory and device memory (which is not cached).

[MAIR_EL1]: http://infocenter.arm.com/help/index.jsp?topic=/com.arm.doc.ddi0500d/CIHDHJBB.html

```rust
impl MemoryManagementUnit {
    /// Setup function for the MAIR_EL1 register.
    fn set_up_mair(&self) {
        // Define the memory types being mapped.
        MAIR_EL1.write(
            // Attribute 1 - Cacheable normal DRAM.
            MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
        MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc +

        // Attribute 0 - Device.
        MAIR_EL1::Attr0_Device::nonGathering_nonReordering_EarlyWriteAck,
        );
    }
```

Afterwards, the [Translation Table Base Register 0 - EL1] is set up with the base address of the
`lvl2` tables and the [Translation Control Register - EL1] is configured:

```rust
// Set the "Translation Table Base Register".
TTBR0_EL1.set_baddr(KERNEL_TABLES.phys_base_address());

self.configure_translation_control();
```

Finally, the `MMU` is turned on through the [System Control Register - EL1]. The last step also
enables caching for data and instructions.

[Translation Table Base Register 0 - EL1]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/ttbr0_el1.rs.html
[Translation Control Register - EL1]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/tcr_el1.rs.html
[System Control Register - EL1]: https://docs.rs/aarch64-cpu/9.0.0/src/aarch64_cpu/registers/sctlr_el1.rs.html

### `kernel.ld`

We need to align the `code` segment to `64 KiB` so that it doesn't overlap with the next section
that needs read/write attributes instead of read/execute attributes:

```ld.s
. = ALIGN(PAGE_SIZE);
__code_end_exclusive = .;
```

This blows up the binary in size, but is a small price to pay considering that it reduces the amount
of static paging entries significantly, when compared to the classical `4 KiB` granule.

## Address translation examples

For educational purposes, a layout is defined which allows to access the `UART` via two different
virtual addresses:
- Since we identity map the whole `Device MMIO` region, it is accessible by asserting its physical
  base address (`0x3F20_1000` or `0xFA20_1000` depending on which RPi you use) after the `MMU` is
  turned on.
- Additionally, it is also mapped into the last `64 KiB` slot in the first `512 MiB`, making it
  accessible through base address `0x1FFF_1000`.

The following block diagram visualizes the underlying translation for the second mapping.

### Address translation using a 64 KiB page descriptor

<img src="../doc/11_page_tables_64KiB.png" alt="Page Tables 64KiB" width="90%">

## Zero-cost abstraction

The MMU init code is again a good example to see the great potential of Rust's zero-cost
abstractions[[1]][[2]] for embedded programming.

Let's take a look again at the piece of code for setting up the `MAIR_EL1` register using the
[aarch64-cpu] crate:

[1]: https://blog.rust-lang.org/2015/05/11/traits.html
[2]: https://ruudvanasseldonk.com/2016/11/30/zero-cost-abstractions
[aarch64-cpu]: https://crates.io/crates/aarch64-cpu

```rust
/// Setup function for the MAIR_EL1 register.
fn set_up_mair(&self) {
    // Define the memory types being mapped.
    MAIR_EL1.write(
        // Attribute 1 - Cacheable normal DRAM.
        MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
    MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc +

    // Attribute 0 - Device.
    MAIR_EL1::Attr0_Device::nonGathering_nonReordering_EarlyWriteAck,
    );
}
```

This piece of code is super expressive, and it makes use of `traits`, different `types` and
`constants` to provide type-safe register manipulation.

In the end, this code sets the first four bytes of the register to certain values according to the
data sheet. Looking at the generated code, we can see that despite all the type-safety and
abstractions, it boils down to two assembly instructions:

```text
   800a8:       529fe089        mov     w9, #0xff04                     // #65284
   800ac:       d518a209        msr     mair_el1, x9
```

## Test it

Turning on virtual memory is now the first thing we do during kernel init:

```rust
unsafe fn kernel_init() -> ! {
    use memory::mmu::interface::MMU;

    if let Err(string) = memory::mmu::mmu().enable_mmu_and_caching() {
        panic!("MMU: {}", string);
    }
```

Later in the boot process, prints about the mappings can be observed:

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
[MP] ⏩ Pushing 64 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.811167] mingo version 0.10.0
[    0.811374] Booting on: Raspberry Pi 3
[    0.811829] MMU online. Special regions:
[    0.812306]       0x00080000 - 0x0008ffff |  64 KiB | C   RO PX  | Kernel code and RO data
[    0.813324]       0x1fff0000 - 0x1fffffff |  64 KiB | Dev RW PXN | Remapped Device MMIO
[    0.814310]       0x3f000000 - 0x4000ffff |  17 MiB | Dev RW PXN | Device MMIO
[    0.815198] Current privilege level: EL1
[    0.815675] Exception handling state:
[    0.816119]       Debug:  Masked
[    0.816509]       SError: Masked
[    0.816899]       IRQ:    Masked
[    0.817289]       FIQ:    Masked
[    0.817679] Architectural timer resolution: 52 ns
[    0.818253] Drivers loaded:
[    0.818589]       1. BCM PL011 UART
[    0.819011]       2. BCM GPIO
[    0.819369] Timer test, spinning for 1 second
[     !!!    ] Writing through the remapped UART at 0x1FFF_1000
[    1.820409] Echoing input now
```

