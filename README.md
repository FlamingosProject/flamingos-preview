# Tutorial 17 - Kernel Symbols

## tl;dr

- To enrich and augment existing and future debugging code, we add support for `kernel symbol`
  lookup.

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Linking Changes](#linking-changes)
  - [Kernel Symbols Tool](#kernel-symbols-tool)
  - [Lookup Code](#lookup-code)
- [Test it](#test-it)

## Introduction

Ever since the first tutorial, it was possible to execute the `make nm` target in order to view all
`kernel symbols`. The kernel itself, however, does not have any means yet to correlate a virtual
address to a symbol during runtime. Gaining this capability would be useful for augmenting
debug-related prints. For example, when the kernel is handling an `exception`, it prints the content
of the `exception link register`, which is the program address where the CPU was executing from when
the exception happened.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow.

Until now, in order to understand to which function or code such an address belongs to, a manual
lookup by the person debugging the issue was necessary. In this tutorial, we are adding a `data
structure` to the kernel which contains _all the symbol names and corresponding address ranges_.
This enables the kernel to print symbol names in existing and future debug-related code, which
improves triaging of issues by humans, because it does away with the manual lookup.

The backtrace support introduced in Chapter 11 also uses this table from this chapter onward, so
panic frames include symbol names as well as instruction addresses.

[`backtracing`]: https://en.wikipedia.org/wiki/Stack_trace

## Implementation

First of all, a new support crate is added under `$ROOT/libraries/debug-symbol-types`. It contains
the definition for `struct Symbol`:

```rust
/// A symbol containing a size.
#[repr(C)]
pub struct Symbol {
    addr_range: Range<usize>,
    name: &'static str,
}
```

To enable the kernel to lookup symbol names, we will add an `array` to the kernel binary that
contains all the kernel symbols. Because we can query the final symbol names and addresses only
_after_ the kernel has been `linked`, the same approach as for the `translation tables` will be
used: The symbols array will be patched into a `placeholder section` of the final kernel `ELF`.

### Linking Changes

In the `kernel.ld` linker script, we define a new section named `kernel_symbols` and give it a size
of `32 KiB`:

```ld.s
    .rodata         : ALIGN(8) { *(.rodata*) } :segment_code
    .got            : ALIGN(8) { *(.got)     } :segment_code
    .kernel_symbols : ALIGN(8) {
        __kernel_symbols_start = .;
        . += 32 * 1024;
    } :segment_code
```

Also, we are providing the start address of the section through the symbol `__kernel_symbols_start`,
which will be used by our `Rust` code later on.

### Kernel Symbols Tool

Under `$ROOT/tools/kernel_symbols_tool`, we are adding a helper tool that is able to dynamically
generate an `array` of all the kernel symbols and patch it into the final kernel `ELF`. In our main
`Makefile`, we are invoking the tool after the translation table generation. In the first step, the
tool generates a temporary `Rust` file that instantiates the symbols array. Here is an example of
how this can look like:

```console
$ head ./target/aarch64-unknown-none-softfloat/release/kernel+ttables_symbols.rs
```
```rust
use debug_symbol_types::Symbol;

# [no_mangle]
# [link_section = ".rodata.symbol_desc"]
static KERNEL_SYMBOLS: [Symbol; 139] = [
    Symbol::new(18446744072635809792, 124, "_start"),
    Symbol::new(18446744072635809920, 8, "BOOT_CORE_ID"),
    Symbol::new(18446744072635809928, 8, "PHYS_KERNEL_TABLES_BASE_ADDR"),
    Symbol::new(18446744072635809936, 80, "_start_rust"),
    Symbol::new(18446744072635813888, 84, "__exception_restore_context"),
    // Many more
```

Next, the _helper crate_ `$ROOT/kernel_symbols` is compiled. This crate contains a single `main.rs`
that just includes the temporary symbols file shown above.

```rust
//! Generation of kernel symbols.

#![no_std]
#![no_main]

#[cfg(feature = "generated_symbols_available")]
include!(env!("KERNEL_SYMBOLS_RS"));
```

`KERNEL_SYMBOLS_RS` is set by the corresponding `build.rs` file. The helper crate has its own
`linker file`, which ensures that just the array and the corresponding strings that it
references are kept:

```ld.s
SECTIONS
{
    .rodata : {
        ASSERT(. > 0xffffffff00000000, "Expected higher half address")

        KEEP(*(.rodata.symbol_desc*))
        . = ALIGN(8);
        *(.rodata*)
    }
}
```

Afterwards, `objcopy` is used to strip the produced helper crate ELF. What remains is a small
`binary blob` that just contains the symbols array and the `names` that are referenced. To ensure
that these references are valid kernel addresses (remember that those are defined as `name: &'static
str`, so basically a pointer to a kernel address), the sub-makefile compiling this helper crate
(`$ROOT/kernel_symbols.mk`) did the following:

It used the `kernel_symbols_tool` to query the virtual address of the `kernel_symbols` **section**
(of the final kernel ELF). This address was then supplied to the linker when the helper crate was
linked (emphasis on the `--section-start=.rodata=` part):

```Makefile
GET_SYMBOLS_SECTION_VIRT_ADDR = $(DOCKER_TOOLS) $(EXEC_SYMBOLS_TOOL) \
    --get_symbols_section_virt_addr $(KERNEL_SYMBOLS_OUTPUT_ELF)

RUSTFLAGS = -C link-arg=--script=$(KERNEL_SYMBOLS_LINKER_SCRIPT) \
    -C link-arg=--section-start=.rodata=$$($(GET_SYMBOLS_SECTION_VIRT_ADDR))
```

This might be a bit convoluted, but the main take away is: This ensures that the start address of
the `.rodata` section of the `kernel_symbols` helper crate is exactly the same address as the
`placeholder section` of the final kernel ELF where the symbols `binary blob` will be patched into.
The latter is the last step done by the tool.

### Lookup Code

In the kernel, we add the file `src/symbols.rs`. It makes the linker-provided symbol
`__kernel_symbols_start` that we saw earlier accesible, and also defines `NUM_KERNEL_SYMBOLS`:

```rust
#[no_mangle]
static NUM_KERNEL_SYMBOLS: u64 = 0;
```

When the `kernel_symbols_tool` patches the symbols blob into the kernel ELF, it also updates this
value to reflect the number of symbols that are available. This is needed for the code that
internally crafts the slice of symbols that the kernel uses for lookup:

```rust
fn kernel_symbol_section_virt_start_addr() -> Address<Virtual> {
    Address::new(unsafe { __kernel_symbols_start.get() as usize })
}

fn num_kernel_symbols() -> usize {
    unsafe {
        // Read volatile is needed here to prevent the compiler from optimizing NUM_KERNEL_SYMBOLS
        // away.
        core::ptr::read_volatile(&NUM_KERNEL_SYMBOLS as *const u64) as usize
    }
}

fn kernel_symbols_slice() -> &'static [Symbol] {
    let ptr = kernel_symbol_section_virt_start_addr().as_usize() as *const Symbol;

    unsafe { slice::from_raw_parts(ptr, num_kernel_symbols()) }
}
```

Lookup is done by just iterating over the slice:

```rust
/// Retrieve the symbol corresponding to a virtual address, if any.
pub fn lookup_symbol(addr: Address<Virtual>) -> Option<&'static Symbol> {
    kernel_symbols_slice()
        .iter()
        .find(|&i| i.contains(addr.as_usize()))
}
```

And that's it for this tutorial. The upcoming tutorial on `backtracing` will put this code to more
prominent use.

## Test it

For now, symbol lookup can be observed in the integration test for synchronous exception handling.
Here, the kernel now also prints the symbol name that corresponds to the value of `ELR_EL1`. In the
following case, this is `kernel_init()`, which is where the the exception is generated in the test:

```console
$ TEST=02_exception_sync_page_fault make test_integration
[...]
         -------------------------------------------------------------------
         🦀 Testing synchronous exception handling by causing a page fault
         -------------------------------------------------------------------

         [    0.002640] Writing to bottom of address space to address 1 GiB...
         [    0.004549] Kernel panic!

         Panic location:
               File 'kernel/src/_arch/aarch64/exception.rs', line 59, column 5

         CPU Exception!

         ESR_EL1: 0x96000004

         ...

         ELR_EL1: 0xffffffffc0001118
               Symbol: kernel_init
```
