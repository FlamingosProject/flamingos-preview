# SD-card driver notes

## Upstream reference

The blocking PIO implementation is based on:

- Repository: `https://github.com/joeferner/rpi-hal`
- Commit: `258c4b9d778f4598eeb05aa97682fe9994fd5a16`
- Source: `src/sd.rs` and `src/emmc.rs`
- License: MIT OR Apache-2.0

The implementation was translated onto this kernel's `MMIODerefWrapper`, high-half MMIO mappings,
timer, synchronization, driver registration, and BSP feature flags. It is not a vendored copy of
the complete HAL.

## Layer boundary

The hardware driver intentionally deals in 512-byte sectors. Filesystem partition discovery,
allocation, directory parsing, and file handles belong above this layer. The intended next boundary
is an implementation of `exfat-embedded::BlockDevice`; that crate performs its own MBR/GPT
discovery, so the kernel does not need a second partition-table dependency.

The adapter reports the capacity decoded from the card's CSD, translates the filesystem's `u64`
LBAs to the controller's `u32` block indices, and rejects out-of-range or incorrectly sized
requests. The filesystem mounts the first MBR type-`0x07` partition (or Microsoft Basic Data GPT
partition) and validates that it contains an exFAT volume.

## Hardware caveats

- Pi 3 and Zero 2 W use the classic controller at physical address `0x3f30_0000`, with the slot
  routed through GPIO48-53 ALT3.
- Pi 4 uses EMMC2 at `0xfe34_0000` and dedicated pads. GPIO48-53 must not be remapped there.
- Pi 4 requires the EMMC2 firmware clock/power setup before card initialization.
- An exact base-clock value is required to keep the identification clock at or below 400 kHz.
  The property mailbox queries clock ID 1 (`EMMC`) on Pi 3-family boards and ID 12 (`EMMC2`) on
  Pi 4 instead of hard-coding a board-frequency guess.
- The mailbox request occupies its own 64-byte-aligned cache line. The driver translates its
  high-half virtual address to a physical address, cleans the request before submission, and
  invalidates the response before reading it.
