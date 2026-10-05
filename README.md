# Tutorial 23 - Load ELF

This chapter reads a static AArch64 executable from exFAT and
prepares an owned `ProgramImage` for process creation. Boot tries
the root-level file `init.elf` and prints its entry address and
loadable segments. Missing media, filesystem errors, and invalid
executables produce a warning and boot continues.

The image is retained in `kernel_main`. It contains the information
needed to place code and data in a future process address space;
this chapter does not yet create page tables, a stack, a saved CPU
context, or a runnable process.

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

`libraries/program-image` is a `no_std` library using `alloc` and
forbidding unsafe code. The kernel exposes it as
`libkernel::program`. Its ELF parser is
[`elf` 0.8](https://docs.rs/elf/0.8.0/elf/), with default features
disabled. Host tests exercise the same loader used by the kernel.

`program::load(filesystem, path, scratch)` borrows a mounted
`exfat_embedded::FileSystem` and reads the file without modifying
the disk. The caller owns the filesystem and sector scratch space.
The returned image owns its segment buffers, so the filesystem can
be dropped or reused immediately. Boot mounts locally; later
process creation can keep a filesystem and call the same function
repeatedly.

`ProgramImage::from_elf(bytes)` provides the parsing and preparation
step separately from disk access. The ELF header and program headers
are parsed using the crate's checked, fixed-endian interfaces.
Section headers, section names, and symbol tables are unnecessary
for loading and are ignored.

The result contains:

- `entry`: the initial instruction's virtual address.
- `segments`: loadable segments sorted by virtual address.
- Each segment's virtual address, ELF alignment, read/write/execute
  permissions, and owned data buffer.
- A buffer of exactly `p_memsz` bytes per segment. The first
  `p_filesz` bytes come from the file; the remainder is zero-filled
  for BSS and other uninitialized data.

The first implementation accepts ELF64, little-endian, AArch64
`ET_EXEC` files. It rejects PIE, relocatable objects, interpreters,
dynamic linking, TLS, and requests for executable stacks. The entry
must identify a complete, four-byte-aligned instruction in the
file-backed portion of an executable segment.

All segment ranges must fit in the file and the lower 48-bit virtual
address range; address zero is rejected. `p_filesz` must not exceed
`p_memsz`. Alignment must be zero, one, or a power of two, and file
offsets must agree with virtual addresses modulo that alignment.
Overlapping segment byte ranges are rejected. These checks follow
the [ELF program-header specification](https://refspecs.linuxfoundation.org/elf/gabi4%2B/ch5.pheader.html).

The loader caps files at 4 MiB, total segment storage at 8 MiB, and
the program-header count at 32. Allocations are fallible. The file
buffer is released after loading; peak storage includes both the
file and the segment buffers. These limits leave room in the
kernel's 16 MiB heap for its other users.

## Try it

Build a small static ELF fixture using Python's standard library:

```sh
python3 tools/make-init-elf.py
cargo run -p program-image --example inspect -- target/init.elf
```

The fixture has a single looping AArch64 instruction, a read-only
executable segment, and a writable segment with initialized data
and a zero-filled tail. It has no libraries or relocations. The
host inspection should report entry `0x100b0` and two segments.

Copy `target/init.elf` into the root of the card's existing exFAT
partition, then build and boot the normal kernel:

```sh
cargo xtask build rpiz2
make chainboot BSP=rpiz2
```

Boot should print `ELF loader: init.elf entry 0x100b0, 2 segments`.
The program is loaded but execution is deferred to process creation.
The Pi 3 and Pi 4 build selectors remain available.

## Validation and remaining work

```sh
cargo test -p program-image
make clippy BSP=rpi3
make clippy BSP=rpi4
make test BSP=rpi3
```

Host tests cover truncated input, unsupported executable formats,
invalid ranges and alignment, segment overlap, entry validation,
allocation limits, owned buffers, zero filling, and read-only exFAT
loading. QEMU tests exclude the boot SD-card path because QEMU lacks
the VideoCore mailbox service used by card initialization.

Process creation must choose page-level mapping policy, compose or
reject segments sharing a page, apply permissions, allocate a user
stack, and construct the initial CPU context. Segment buffers here
are ordinary heap allocations, not installed virtual mappings.
The existing SD-card limitations described in Chapter 22 remain.

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
