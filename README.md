# Tutorial 22 - SD Card

## tl;dr

- The BSP maps the Raspberry Pi SD controller (`EMMC` on Pi 3-family boards and `EMMC2` on Pi 4).
- GPIO48-53 are routed to the Arasan controller on Pi 3 and Zero 2 W builds.
- A new blocking, `no_std` PIO driver performs card identification and 512-byte sector reads and
  writes.
- The driver exposes sectors rather than filesystem operations so an exFAT implementation can be
  layered on top without coupling the BSP to one filesystem crate.

## Implementation status

The first implementation slice is adapted from `joeferner/rpi-hal` 0.6.0, specifically commit
`258c4b9d778f4598eeb05aa97682fe9994fd5a16`. Only the native SD blocking path was brought over;
`rpi-hal`'s runtime, PAC, MMU, interrupt, DMA, and filesystem adapters are not dependencies.

Implemented:

- SDHCI register mapping through the kernel's existing MMIO allocator.
- Pi 3/Zero 2 W GPIO mux and pull-up setup; Pi 4 uses dedicated EMMC2 pads.
- `CMD0`/`CMD8`/`ACMD41`/`CMD2`/`CMD3`/`CMD7` card initialization.
- Best-effort four-bit bus negotiation through `ACMD51` and `ACMD6`.
- Blocking `CMD17` sector reads and `CMD24` sector writes.
- Time-bounded controller polling and diagnostic command/interrupt errors.
- VideoCore property-mailbox clock discovery, including the cache maintenance and virtual-to-
  physical address translation required by this kernel's MMU setup.
- SD power-domain setup and Pi 4 EMMC2 clock enablement through firmware.

Still required before hardware acceptance:

- CSD decoding so the block layer can report the card's sector count.
- Real-hardware tests on Pi 3, Zero 2 W, and Pi 4. In particular, upstream has not yet validated its
  Pi 3-family path on Zero 2 W hardware.
- Multi-block transfers and, later, DMA/interrupt-driven I/O.
- The filesystem-facing block-device adapter and exFAT dependency.

## Using the driver

The controller and property mailbox are registered during BSP initialization, but removable-media
discovery is not part of kernel boot. Initialize and use the card explicitly:

```rust,ignore
let sd = bsp::driver::emmc();
sd.initialize(bsp::driver::mailbox())?;

let mut sector = [0_u8; 512];
sd.read_block(0, &mut sector)?;
```

Keeping discovery explicit means an empty or faulty SD slot does not prevent the kernel from
booting far enough to report the error over UART.

## Test it

The driver currently has compile- and boot-level coverage:

```text
cargo xtask build rpi3
cargo xtask build rpi4
make test_boot BSP=rpi3
```

The QEMU smoke test confirms the new MMIO mapping and driver registration do not regress boot. It
does not claim that QEMU or real hardware has exercised card initialization or data transfer yet.

The normal hardware image also runs a sector-zero read test during boot:

```text
make BSP=rpiz2
```

The test powers and initializes the card, reads logical sector 0, and prints its IEEE CRC-32 and
negotiated bus width over UART. An empty slot is reported as `no card present` and boot continues.
The read is excluded only from QEMU test builds, where no VideoCore firmware property service is
available.
