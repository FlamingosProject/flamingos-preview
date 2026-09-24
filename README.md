# Tutorial 22 - SD Card and exFAT

## tl;dr

- The BSP maps and initializes the Raspberry Pi SD host controller: `EMMC` on Pi 3 and Zero 2 W,
  and `EMMC2` on Pi 4.
- A blocking, `no_std` PIO driver identifies the card, determines its capacity, negotiates a
  four-bit bus when possible, and reads or writes one 512-byte logical sector at a time.
- A small adapter implements `exfat_embedded::BlockDevice` without making the hardware driver
  depend on a filesystem.
- The boot demonstration mounts an exFAT partition, opens `123.txt`, and verifies its contents.

## Table of Contents

- [Introduction](#introduction)
- [Implementation](#implementation)
  - [Three Layers](#three-layers)
  - [Selecting and Powering the Controller](#selecting-and-powering-the-controller)
  - [Initializing the Card](#initializing-the-card)
  - [Capacity and Addressing](#capacity-and-addressing)
  - [Single-Sector PIO](#single-sector-pio)
  - [The Filesystem Adapter](#the-filesystem-adapter)
  - [Mounting exFAT](#mounting-exfat)
- [Test it](#test-it)
- [Current Limitations](#current-limitations)

## Introduction

The kernel can now get persistent data from the same SD card that booted the Raspberry Pi. This is
the first chapter that treats the card as a runtime storage device rather than as something used
only by the firmware before the kernel starts.

The developer workflows introduced earlier remain available. Use `make chainboot` to send the
normal kernel, `CHAINLOADER=1 make` to build the persistent EL2/MMU-off loader as
`chainloader8.img`, and `make jtagboot`, `make openocd`, `make gdb`, or `make gdb-opt0` for the
Chapter 08 hardware-debugging workflow. Select the target with `BSP=rpiz2`, `BSP=rpi3`, or
`BSP=rpi4`.

Storage is a useful place to preserve abstraction boundaries. The SD controller transfers numbered
sectors; a partition table describes regions within those sectors; a filesystem gives names and
structure to the bytes inside one region. Keeping those jobs separate lets a future ELF loader ask
for a file without knowing anything about SD commands or MBR entries.

This chapter's controller driver is adapted from the native-SD blocking path in
`joeferner/rpi-hal` 0.6.0, at commit `258c4b9d778f4598eeb05aa97682fe9994fd5a16`. Flamingos uses
its own MMIO mapping, synchronization, timer, GPIO, and VideoCore mailbox infrastructure; it does
not depend on that HAL at runtime.

## Implementation

### Three Layers

The path from a filename to the hardware is deliberately short but layered:

```text
exfat_embedded::FileSystem
            |
            | exfat_embedded::BlockDevice (u64 LBA, byte slice)
            v
       SdCardBlockDevice
            |
            | EMMC API (u32 block index, [u8; 512])
            v
       EMMC / EMMC2 host controller
            |
            v
          SD card
```

The EMMC driver knows nothing about partitions or filesystems. `SdCardBlockDevice` only translates
between the two block APIs and validates their ranges and buffer sizes. `exfat-embedded` performs
partition discovery and interprets the exFAT structures.

All three layers are `no_std`. The demonstration also avoids allocation for filesystem scratch
space by lending the exFAT implementation one stack-allocated sector.

### Selecting and Powering the Controller

Raspberry Pi models do not all connect the card slot in the same way:

- Pi 3 and Zero 2 W use the classic Arasan `EMMC` controller. GPIO48-53 are switched to ALT3 and
  configured with pull-ups before the controller is used.
- Pi 4 uses `EMMC2` and dedicated SD-card pins, so it does not need that GPIO mux operation. Its
  clock and standard SDHCI bus-power bits must be enabled explicitly.

The controller's base clock is not assumed. The ARM asks the VideoCore firmware for the active EMMC
clock rate through the property mailbox, then calculates SDHCI divisors from the returned value.
This matters because the divisor is meaningful only relative to the clock actually supplied to the
peripheral.

Card identification starts at no more than 400 kHz, as required by the SD protocol. Once the card
has been identified and its registers read, the driver moves to a conservative 25 MHz transfer
clock. Controller waits and command/data waits are bounded, so an absent or unresponsive card
causes an error instead of an infinite boot hang.

### Initializing the Card

Initialization is a protocol exchange, not just a controller reset:

1. `CMD0` resets the card to its idle state.
2. `CMD8` checks that it understands the host's interface condition.
3. Repeated `CMD55` + `ACMD41` calls advertise the supported voltage and wait for power-up to
   complete. The response also tells us whether the card uses block addressing.
4. `CMD2` reads the card identification register, and `CMD3` assigns a relative card address.
5. `CMD9` reads the card-specific data register (CSD), which describes capacity.
6. `CMD7` selects the card for data transfer. Byte-addressed cards also receive `CMD16` to select
   512-byte blocks.
7. `ACMD51` reads the SD configuration register. If the card advertises four-bit transfers,
   `ACMD6` switches the card and the matching controller bit is enabled.

The last step is best effort. A card that supports only the original one-bit bus can still be used;
it is merely slower.

### Capacity and Addressing

The CSD has two layouts used by cards supported here. Version 1 describes capacity in terms of a
native block length, a device-size field, and a multiplier. Version 2, used by SDHC and SDXC cards,
has a larger device-size field whose units are fixed at 512 KiB. The driver decodes either layout
to a single `u64` count of 512-byte logical sectors.

The public block API always uses 512-byte sectors, but the command argument depends on the card:

- SDHC and SDXC cards are block addressed, so `CMD17` argument 1 means logical sector 1.
- Standard-capacity cards are byte addressed, so logical sector 1 is sent as byte offset 512.

This distinction remains private to the controller driver. Callers use logical block addresses in
both cases.

### Single-Sector PIO

`read_block()` issues `CMD17`; `write_block()` issues `CMD24`. Transfers are currently synchronous
and use programmed I/O (PIO): the CPU waits for the controller's FIFO-ready indication and copies
the sector as 128 little-endian 32-bit words. A write does not return until the controller reports
`DATA_DONE`, which means the card has completed programming the sector.

The controller is protected by an IRQ-safe lock. That serializes card state and transfers, but it
also means the current API is intentionally blocking. Multi-block commands, DMA, and interrupt-
driven completion are performance work for a later revision, not prerequisites for reading small
program files.

### The Filesystem Adapter

`exfat-embedded` expects this interface:

```rust,ignore
trait BlockDevice {
    fn sector_size(&self) -> usize;
    fn sector_count(&self) -> u64;
    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), Self::Error>;
    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
}
```

`SdCardBlockDevice` wraps the initialized EMMC instance and caches its CSD-derived capacity. It
requires exactly one 512-byte sector per operation, rejects LBAs outside that capacity, and rejects
LBAs that cannot be represented by the controller's current `u32` block index. `flush()` has no
queued work because every controller write is already synchronous.

### Mounting exFAT

`FileSystem::mount()` reads sector 0 itself and locates the first suitable partition. For an MBR it
looks for partition type `0x07`; for GPT it looks for a Microsoft Basic Data partition. It then
validates the exFAT boot region inside that partition. No separate partition-table crate is needed.

This distinction explains the two reads visible in the boot demonstration. The kernel first reads
absolute sector 0 and logs its CRC-32 as a compact transport-level fingerprint. The filesystem then
starts from the same whole-disk block device, discovers partition 2, and interprets LBAs relative
to that partition.

The demonstration opens `123.txt` in the exFAT root directory and checks for the four bytes
`123\n`. The lookup and read are temporary boot-time acceptance tests; later code can reuse the
same storage layers without hard-coding this filename.

## Test it

Prepare an SD card with the normal FAT boot partition and an exFAT partition containing a root-level
file named `123.txt`. Its contents must be exactly the bytes `31 32 33 0a` (`123` followed by a
newline). For example, with the exFAT volume mounted at `/mnt/exfat`:

```sh
printf '123\n' > /mnt/exfat/123.txt
```

If the card uses an MBR, give the exFAT partition type `0x07`. Then build the normal kernel for the
board in use, for example:

```text
cargo xtask build rpiz2
make chainboot BSP=rpiz2
```

`make chainboot` uses `/dev/ttyUSB0` by default; set `DEV_SERIAL` if the adapter appears elsewhere.
On success the UART log includes messages of this form:

```text
SD card test: 124735488 sectors, sector 0 CRC-32 = 0x595157c4, four-bit bus = true
exFAT test: mounted partition at LBA 123456 (12345678 sectors)
exFAT test: 123.txt contains expected data
```

The exact capacity, partition geometry, and CRC depend on the card. With no card installed, boot
continues after reporting `SD card test: no card present`.

The QEMU test build excludes the hardware smoke test because QEMU does not provide the Raspberry
Pi VideoCore property-mailbox service used to power and clock this controller. It still checks that
the new driver and mappings do not regress the rest of the kernel:

```text
make clippy BSP=rpi3
make clippy BSP=rpi4
make test BSP=rpi3
```

## Current Limitations

- The data path transfers one sector at a time using blocking PIO. There is no multi-block, DMA, or
  interrupt-driven I/O yet.
- Card insertion and removal are not monitored. Filesystem users must define a lifecycle before
  supporting hot-plug or recovery from removal during a transfer.
- Initialization requires the modern `CMD8` handshake. Very old, pre-SD-2.0 cards are not
  supported.
- CSD version 3 (SDUC) and block indices beyond `u32` are rejected.
- The boot demonstration has exercised reading. The write implementation exists, but destructive
  on-card write testing and power-loss behavior need dedicated tests before the kernel relies on
  filesystem mutation.
