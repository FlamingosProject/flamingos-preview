# Tutorial 14 - Virtual Memory Part 2: MMIO Remap

## tl;dr

- We introduce a first set of changes which is eventually needed for separating `kernel` and `user`
  address spaces.
- The memory mapping strategy gets more sophisticated as we do away with `identity mapping` the
  whole of the board's address space.
- Instead, only ranges that are actually needed are mapped:
    - The `kernel binary` stays `identity mapped` for now.
    - Device `MMIO regions` are remapped lazily (to a special reserved virtual address region).

## Table of Contents

- [Tutorial 14 - Virtual Memory Part 2: MMIO Remap](#tutorial-14---virtual-memory-part-2-mmio-remap)
  - [tl;dr](#tldr)
  - [Table of Contents](#table-of-contents)
  - [Introduction](#introduction)
  - [Implementation](#implementation)
    - [A New Mapping API in `src/memory/mmu/translation_table.rs`](#a-new-mapping-api-in-srcmemorymmutranslation_tablers)
    - [The new APIs in action](#the-new-apis-in-action)
    - [MMIO Virtual Address Allocation](#mmio-virtual-address-allocation)
    - [Supporting Changes](#supporting-changes)
  - [Test it](#test-it)

## Introduction

This tutorial is a first step of many needed for enabling `userspace applications` (which we
hopefully will have some day in the very distant future).

For this, one of the features we want is a clean separation of `kernel` and `user` address spaces.
Fortunately, `ARMv8` has convenient architecture support to realize this. The following text and
pictue gives some more motivation and technical information. It is quoted from the _[ARM Cortex-A
Series Programmer’s Guide for ARMv8-A], Chapter 12.2, Separation of kernel and application Virtual
Address spaces_:

> Operating systems typically have a number of applications or tasks running concurrently. Each of
> these has its own unique set of translation tables and the kernel switches from one to another as
> part of the process of switching context between one task and another. However, much of the memory
> system is used only by the kernel and has fixed virtual to Physical Address mappings where the
> translation table entries rarely change. The ARMv8 architecture provides a number of features to
> efficiently handle this requirement.
>
> The table base addresses are specified in the Translation Table Base Registers `TTBR0_EL1` and
> `TTBR1_EL1`. The translation table pointed to by `TTBR0` is selected when the upper bits of the VA
> are all 0. `TTBR1` is selected when the upper bits of the VA are all set to 1. [...]
>
> Figure 12-4 shows how the kernel space can be mapped to the most significant area of memory and
> the Virtual Address space associated with each application mapped to the least significant area of
> memory. However, both of these are mapped to a much smaller Physical Address space.

<p align="center">
    <img src="../doc/15_kernel_user_address_space_partitioning.png" height="500" align="center">
</p>

This approach is also sometimes called a "[higher half kernel]". To eventually achieve this
separation, this tutorial makes a start by changing the following things:

1. Instead of bulk-`identity mapping` the whole of the board's address space, only the particular
   parts that are needed will be mapped.
1. For now, the `kernel binary` stays identity mapped. This will be changed in the coming tutorials
   as it is a quite difficult and peculiar exercise to remap the kernel.
1. Device `MMIO regions` are lazily remapped during device driver bringup (using the new
   `DriverManage` function `instantiate_drivers()`).
   1. A dedicated region of virtual addresses that we reserve using `BSP` code and the `linker
      script` is used for this.
1. We keep using `TTBR0` for the kernel translation tables for now. This will be changed when we
   remap the `kernel binary` in the coming tutorials.

[ARM Cortex-A Series Programmer’s Guide for ARMv8-A]: https://developer.arm.com/documentation/den0024/latest/
[higher half kernel]: https://wiki.osdev.org/Higher_Half_Kernel

## Implementation

Until now, the whole address space of the board was identity mapped at once. The **architecture**
(`src/_arch/_/memory/**`) and **bsp** (`src/bsp/_/memory/**`) parts of the kernel worked
together directly while setting up the translation tables, without any indirection through **generic
kernel code** (`src/memory/**`).

The way it worked was that the `architectural MMU code` would query the `bsp code` about the start
and end of the physical address space, and any special regions in this space that need a mapping
that _is not_ normal chacheable DRAM. It would then go ahead and map the whole address space at once
and never touch the translation tables again during runtime.

Changing in this tutorial, **architecture** and **bsp** code will no longer autonomously create the
virtual memory mappings. Instead, this is now orchestrated by the kernel's **generic MMU subsystem
code**.

### A New Mapping API in `src/memory/mmu/translation_table.rs`

First, we define an interface for operating on `translation tables`:

```rust
/// Translation table operations.
pub trait TranslationTable {
    /// Anything that needs to run before any of the other provided functions can be used.
    ///
    /// # Safety
    ///
    /// - Implementor must ensure that this function can run only once or is harmless if invoked
    ///   multiple times.
    fn init(&mut self);

    /// The translation table's base address to be used for programming the MMU.
    fn phys_base_address(&self) -> Address<Physical>;

    /// Map the given virtual memory region to the given physical memory region.
    unsafe fn map_at(
        &mut self,
        virt_region: &MemoryRegion<Virtual>,
        phys_region: &MemoryRegion<Physical>,
        attr: &AttributeFields,
    ) -> Result<(), &'static str>;
}
```

In order to enable the generic kernel code to manipulate the kernel's translation tables, they must
first be made accessible. Until now, they were just a "hidden" struct in the `architectural` MMU
driver (`src/arch/.../memory/mmu.rs`). This made sense because the MMU driver code was the only code
that needed to be concerned with the table data structure, so having it accessible locally
simplified things.

Since the tables need to be exposed to the rest of the kernel code now, it makes sense to move them
to `BSP` code. Because ultimately, it is the `BSP` that is defining the translation table's
properties, such as the size of the virtual address space that the tables need to cover.

They are now defined in the global instances region of `src/bsp/.../memory/mmu.rs`. To control
access, they are  guarded by an `InitStateLock`.

```rust
//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

/// The kernel translation tables.
static KERNEL_TABLES: InitStateLock<KernelTranslationTable> =
    InitStateLock::new(KernelTranslationTable::new());
```

The struct `KernelTranslationTable` is a type alias defined in the same file, which in turn gets its
definition from an associated type of type `KernelVirtAddrSpace`, which itself is a type alias of
`memory::mmu::AddressSpace`. I know this sounds horribly complicated, but in the end this is just
some layers of `const generics` whose implementation is scattered between `generic` and `arch` code.
This is done to (1) ensure a sane compile-time definition of the translation table struct (by doing
various bounds checks), and (2) to separate concerns between generic `MMU` code and specializations
that come from the `architectural` part.

In the end, these tables can be accessed by calling `bsp::memory::mmu::kernel_translation_tables()`:

```rust
/// Return a reference to the kernel's translation tables.
pub fn kernel_translation_tables() -> &'static InitStateLock<KernelTranslationTable> {
    &KERNEL_TABLES
}
```

Finally, the generic kernel code (`src/memory/mmu.rs`) now provides a couple of memory mapping
functions that access and manipulate this instance. They  are exported for the rest of the kernel to
use:

```rust
/// Raw mapping of a virtual to physical region in the kernel translation tables.
///
/// Prevents mapping into the MMIO range of the tables.
pub unsafe fn kernel_map_at(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) -> Result<(), &'static str>;

/// MMIO remapping in the kernel translation tables.
///
/// Typically used by device drivers.
pub unsafe fn kernel_map_mmio(
    name: &'static str,
    mmio_descriptor: &MMIODescriptor,
) -> Result<Address<Virtual>, &'static str>;

/// Map the kernel's binary. Returns the translation table's base address.
pub unsafe fn kernel_map_binary() -> Result<Address<Physical>, &'static str>;

/// Enable the MMU and data + instruction caching.
pub unsafe fn enable_mmu_and_caching(
    phys_tables_base_addr: Address<Physical>,
) -> Result<(), MMUEnableError>;
```

### The new APIs in action

`kernel_map_binary()` and `enable_mmu_and_caching()` are used early in `kernel_init()` to set up
virtual memory:

```rust
let phys_kernel_tables_base_addr = match memory::mmu::kernel_map_binary() {
    Err(string) => panic!("Error mapping kernel binary: {}", string),
    Ok(addr) => addr,
};

if let Err(e) = memory::mmu::enable_mmu_and_caching(phys_kernel_tables_base_addr) {
    panic!("Enabling MMU failed: {}", e);
}
```

Both functions internally use `bsp` and `arch` specific code to achieve their goals. For example,
`memory::mmu::kernel_map_binary()` itself wraps around a `bsp` function of the same name
(`bsp::memory::mmu::kernel_map_binary()`):

```rust
/// Map the kernel binary.
pub unsafe fn kernel_map_binary() -> Result<(), &'static str> {
    generic_mmu::kernel_map_at(
        "Kernel boot-core stack",
        // omitted for brevity.
    )?;

    generic_mmu::kernel_map_at(
        "Kernel code and RO data",
        &virt_code_region(),
        &kernel_virt_to_phys_region(virt_code_region()),
        &AttributeFields {
            mem_attributes: MemAttributes::CacheableDRAM,
            acc_perms: AccessPermissions::ReadOnly,
            execute_never: false,
        },
    )?;

    generic_mmu::kernel_map_at(
        "Kernel data and bss",
        // omitted for brevity.
    )?;

    Ok(())
}
```

Another user of the new APIs is the **driver subsystem**. As has been said in the introduction, the
goal is to remap the `MMIO` regions of the drivers. To achieve this in a seamless way, some changes
to the architecture of the driver subsystem were needed.

Until now, the drivers were `static instances` which had their `MMIO addresses` statically set in
the constructor. This was fine, because even if virtual memory was activated, only `identity
mapping` was used, so the hardcoded addresses would be valid with and without the MMU being active.

With `remapped MMIO addresses`, this is not possible anymore, since the remapping will only happen
at runtime. Therefore, the new approach is to defer the whole instantiation of the drivers until the
remapped addresses are known. To achieve this, in `src/bsp/raspberrypi/drivers.rs`, the static
driver instances are now wrapped into a `MaybeUninit` (and are also `mut` now):

```rust
static mut PL011_UART: MaybeUninit<device_driver::PL011Uart> = MaybeUninit::uninit();
static mut GPIO: MaybeUninit<device_driver::GPIO> = MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi3")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::InterruptController> =
    MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi4")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::GICv2> = MaybeUninit::uninit();
```

Accordingly, new dedicated `instantiate_xyz()` functions have been added, which will be called by
the corresponding `driver_xyz()` functions. Here is an example for the `UART`:

```rust
/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_uart() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::PL011_UART_START, mmio::PL011_UART_SIZE);
    let virt_addr =
        memory::mmu::kernel_map_mmio(device_driver::PL011Uart::COMPATIBLE, &mmio_descriptor)?;

    PL011_UART.write(device_driver::PL011Uart::new(virt_addr));

    Ok(())
}
```

```rust
/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_uart() -> Result<(), &'static str> {
    instantiate_uart()?;

    let uart_descriptor = generic_driver::DeviceDriverDescriptor::new(
        PL011_UART.assume_init_ref(),
        Some(post_init_uart),
        Some(exception::asynchronous::irq_map::PL011_UART),
    );
    generic_driver::driver_manager().register_driver(uart_descriptor);

    Ok(())
}
```

The code shows that an `MMIODescriptor` is created first, and then used to remap the MMIO region
using `memory::mmu::kernel_map_mmio()`. This function will be discussed in detail in the next
chapter. What's important for now is that it returns the new `Virtual Address` of the remapped MMIO
region. The constructor of the `UART` driver now also expects a virtual address.

Next, a new instance of the `PL011Uart` driver is created, and written into the `PL011_UART` global
variable (remember, it is defined as `MaybeUninit<device_driver::PL011Uart> =
MaybeUninit::uninit()`). Meaning, after this line of code, `PL011_UART` is properly initialized.
Only then, the driver is registered with the kernel and thus becomes accessible for the first time.
This ensures that nobody can use the UART before its memory has been initialized properly.

### MMIO Virtual Address Allocation

Getting back to the remapping part, let's peek inside `memory::mmu::kernel_map_mmio()`. We can see
that a `virtual address region` is obtained from an `allocator` before remapping:

```rust
pub unsafe fn kernel_map_mmio(
    name: &'static str,
    mmio_descriptor: &MMIODescriptor,
) -> Result<Address<Virtual>, &'static str> {

    // omitted

        let virt_region =
            page_alloc::kernel_mmio_va_allocator().lock(|allocator| allocator.alloc(num_pages))?;

        kernel_map_at_unchecked(
            name,
            &virt_region,
            &phys_region,
            &AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        )?;

    // omitted
}
```

This allocator is defined and implemented in the added file `src/memory/mmu/page_alloc.rs`. Like
other parts of the mapping code, its implementation makes use of the newly introduced
`PageAddress<ATYPE>` and `MemoryRegion<ATYPE>` types (in
[`src/memory/mmu/types.rs`](kernel/src/memory/mmu/types.rs)), but apart from that is rather straight
forward. Therefore, it won't be covered in details here.

The more interesting question is: How does the allocator get to learn which VAs it can use?

This is happening in the following function, which gets called as part of
`memory::mmu::post_enable_init()`, which in turn gets called in `kernel_init()` after the MMU has
been turned on.

```rust
/// Query the BSP for the reserved virtual addresses for MMIO remapping and initialize the kernel's
/// MMIO VA allocator with it.
fn kernel_init_mmio_va_allocator() {
    let region = bsp::memory::mmu::virt_mmio_remap_region();

    page_alloc::kernel_mmio_va_allocator().lock(|allocator| allocator.init(region));
}
```

Again, it is the `BSP` that provides the information. The `BSP` itself indirectly gets it from the
linker script. In it, we have defined an `8 MiB` region right after the `.data` segment:

```ld.s
__data_end_exclusive = .;

/***********************************************************************************************
* MMIO Remap Reserved
***********************************************************************************************/
__mmio_remap_start = .;
. += 8 * 1024 * 1024;
__mmio_remap_end_exclusive = .;

ASSERT((. & PAGE_MASK) == 0, "MMIO remap reservation is not page aligned")
```

The two symbols `__mmio_remap_start` and `__mmio_remap_end_exclusive` are used by the `BSP` to learn
the VA range.

### Supporting Changes

There's a couple of changes more not covered in this tutorial text, but the reader should ideally
skim through them:

- [`src/memory.rs`](kernel/src/memory.rs) and
  [`src/memory/mmu/types.rs`](kernel/src/memory/mmu/types.rs) introduce supporting types,
  like`Address<ATYPE>`, `PageAddress<ATYPE>` and `MemoryRegion<ATYPE>`. It is worth reading their
  implementations.
- [`src/memory/mmu/mapping_record.rs`](kernel/src/memory/mmu/mapping_record.rs) provides the generic
  kernel code's way of tracking previous memory mappings for use cases such as reusing existing
  mappings (in case of drivers that have their MMIO ranges in the same `64 KiB` page) or printing
  mappings statistics.

## Test it

When you load the kernel, you can now see that the driver's MMIO virtual addresses start right after
the `.data` section:

Raspberry Pi 3:

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
[MP] ⏩ Pushing 65 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.740694] mingo version 0.14.0
[    0.740902] Booting on: Raspberry Pi 3
[    0.741357] MMU online:
[    0.741649]       -------------------------------------------------------------------------------------------------------------------------------------------
[    0.743393]                         Virtual                                   Physical               Size       Attr                    Entity
[    0.745138]       -------------------------------------------------------------------------------------------------------------------------------------------
[    0.746883]       0x0000_0000_0000_0000..0x0000_0000_0007_ffff --> 0x00_0000_0000..0x00_0007_ffff | 512 KiB | C   RW XN | Kernel boot-core stack
[    0.748486]       0x0000_0000_0008_0000..0x0000_0000_0008_ffff --> 0x00_0008_0000..0x00_0008_ffff |  64 KiB | C   RO X  | Kernel code and RO data
[    0.750099]       0x0000_0000_0009_0000..0x0000_0000_000e_ffff --> 0x00_0009_0000..0x00_000e_ffff | 384 KiB | C   RW XN | Kernel data and bss
[    0.751670]       0x0000_0000_000f_0000..0x0000_0000_000f_ffff --> 0x00_3f20_0000..0x00_3f20_ffff |  64 KiB | Dev RW XN | BCM PL011 UART
[    0.753187]                                                                                                             | BCM GPIO
[    0.754638]       0x0000_0000_0010_0000..0x0000_0000_0010_ffff --> 0x00_3f00_0000..0x00_3f00_ffff |  64 KiB | Dev RW XN | BCM Interrupt Controller
[    0.756264]       -------------------------------------------------------------------------------------------------------------------------------------------
```

Raspberry Pi 4:

```console
$ BSP=rpi4 make chainboot
[...]
Minipush 1.0

[MP] ⏳ Waiting for /dev/ttyUSB0
[MP] ✅ Serial connected
[MP] 🔌 Please power the target now

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 4

[ML] Requesting binary
[MP] ⏩ Pushing 65 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.736136] mingo version 0.14.0
[    0.736170] Booting on: Raspberry Pi 4
[    0.736625] MMU online:
[    0.736918]       -------------------------------------------------------------------------------------------------------------------------------------------
[    0.738662]                         Virtual                                   Physical               Size       Attr                    Entity
[    0.740406]       -------------------------------------------------------------------------------------------------------------------------------------------
[    0.742151]       0x0000_0000_0000_0000..0x0000_0000_0007_ffff --> 0x00_0000_0000..0x00_0007_ffff | 512 KiB | C   RW XN | Kernel boot-core stack
[    0.743754]       0x0000_0000_0008_0000..0x0000_0000_0008_ffff --> 0x00_0008_0000..0x00_0008_ffff |  64 KiB | C   RO X  | Kernel code and RO data
[    0.745368]       0x0000_0000_0009_0000..0x0000_0000_000d_ffff --> 0x00_0009_0000..0x00_000d_ffff | 320 KiB | C   RW XN | Kernel data and bss
[    0.746938]       0x0000_0000_000e_0000..0x0000_0000_000e_ffff --> 0x00_fe20_0000..0x00_fe20_ffff |  64 KiB | Dev RW XN | BCM PL011 UART
[    0.748455]                                                                                                             | BCM GPIO
[    0.749907]       0x0000_0000_000f_0000..0x0000_0000_000f_ffff --> 0x00_ff84_0000..0x00_ff84_ffff |  64 KiB | Dev RW XN | GICv2 GICD
[    0.751380]                                                                                                             | GICV2 GICC
[    0.752853]       -------------------------------------------------------------------------------------------------------------------------------------------
```

