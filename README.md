# Tutorial 17 - Kernel Symbols

## Introduction

`make nm` shows addresses and symbols on the host. During an
exception, the kernel itself sees addresses such as `ELR_EL1`,
the exception link register. The kernel can now print the
corresponding function names from the ELF it booted. The
backtrace support introduced in Chapter 11 uses the same
lookup for panic frames.

## Retaining the ELF

Stage 1 uses the ELF program headers to load the kernel into
RAM. It also keeps the complete source ELF in its reserved
physical range at 64–96 MiB. Stage 2 maps the pages containing
the ELF read-only and execute-never at
`0xffff_ffff_c200_0000`. That mapping keeps the metadata
accessible once the kernel switches to higher-half addresses.

The build removes debug sections from the normal boot payload
while keeping the ordinary ELF symbol table, its associated
string table, and unwind data. Section metadata can stay in
the retained source ELF while the loadable segments occupy
their linked locations. The symbol values describe those
linked virtual addresses.

## Lookup in the kernel

`kernel/src/symbols.rs` asks `kernel-image` to read the
retained ELF. The lookup scans defined function symbols with
nonempty address ranges. An address belongs to a function
when it is at or beyond the function's start and below the
end. If ranges overlap, the lookup picks the closest
containing start and then the narrowest range at that start.

The symbol's name is an offset into the associated string
table. Lookup borrows that raw name from the retained ELF.
Exception and backtrace printing use `rustc-demangle` while
formatting the name, without allocating heap memory. When
metadata is absent or invalid, the diagnostic still has the
numeric address.

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
TEST=02_exception_sync_page_fault make test_integration BSP=rpi3
```

The test triggers a page fault and prints the exception's
`ELR_EL1` together with the function name `kernel_init`.

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
