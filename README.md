# Tutorial 15 - Virtual Memory Part 3: Kernel ELF Boot

## Introduction

The Raspberry Pi firmware starts the bootstrap at physical
address `0x80000` (512 KiB), with address translation disabled.
Moving the kernel into the upper virtual address range will
require a different link address. This chapter separates the
code that runs at the physical entry from the kernel that will
run at its linked addresses. It keeps the stage-2 kernel
identity-mapped with `TTBR0_EL1`. Chapter 16 moves it to
`TTBR1_EL1`.

## Load addresses, link addresses, and PIC

The link address tells the kernel where its code and data will
appear once translation is active. The physical load address
tells the bootstrap where those bytes belong in RAM.

On AArch64, an `ADRP` and `ADD` pair can form the address of a
nearby object relative to the current instruction's page. For
a page-aligned move, the pair moves that address by the same
offset as the code. An absolute address encoded in instructions
or stored in a global offset table keeps its linked value.
Source-level address expressions can result in either form.

Position-independent code (PIC) can run after a move when its
references still resolve. A compiler's PIC mode can also
introduce global offset table (GOT) references that a dynamic
loader would normally resolve. Early boot runs without that
runtime. The stage-1 assembly controls its address calculations
and moves the loader to its linked physical location before
calling Rust. The stage-2 assembly also distinguishes physical
addresses used before translation from linked addresses used
upon entering EL1.

## Stage 1 loads the ELF

The bootstrap receives an ELF over UART or finds one embedded
in the firmware image. The serial transfer still uses the
existing size-prefixed protocol. The ELF program headers give
stage 1 the file extent, physical destination, and memory
extent of each loadable segment. The `kernel-image` library
validates the static, little-endian AArch64 ELF before any
segment writes. Stage 1 copies the file-backed bytes and
zero-fills the remaining memory in each segment.

| Physical range | Use |
| --- | --- |
| 0–64 KiB | Preserved firmware parking code and release slots |
| 64–512 KiB | Stage-2 boot stack |
| 512 KiB–32 MiB | Stage-2 code and data load area |
| 32–64 MiB | Relocated stage 1, stack, and copied device tree |
| 64–96 MiB | Retained source ELF, up to 32 MiB |

Stage 1 checks segment bounds, overlap and the executable
entry before handing off to the physical entry at `0x80000`
with the MMU off. The handoff passes the device tree pointer
in `x0`, the retained ELF base in `x1`, and the ELF byte count
in `x2`.

## Stage 2 builds the translation tables

The kernel linker script describes the physical locations and
virtual bounds of the code, data, and boot stack. Stage 2 reads
those bounds from its physical bootstrap and fills the kernel
page tables while the MMU is off. It maps code and read-only
data as executable, data and the stack as writable and
execute-never, and the retained ELF read-only and execute-never.
The build produces the linked ELF and packages it as needed.
Translation tables are constructed during boot.

The handoff to EL1 uses the linked addresses for the kernel
entry and its stack. Keeping the physical and virtual bounds
explicit here makes the higher-half transition in the next
chapter possible.

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

The ELF shows equal virtual and physical addresses for its
loadable segments. The boot log reports identity mappings
once the kernel enables the MMU.

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
