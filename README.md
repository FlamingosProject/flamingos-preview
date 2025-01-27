# Tutorial 05 - Drivers: GPIO and UART

## tl;dr

- Drivers for the real `UART` and the `GPIO` controller are added.
- **For the first time, we will be able to run the code on the real hardware** (scroll down for
  instructions).

## Introduction

Now that we enabled safe globals in the previous tutorial, the infrastructure is laid for adding the
first real device drivers. We throw out the magic QEMU console and introduce a `driver manager`,
which allows the `BSP` to register device drivers with the `kernel`.

## Driver Manager

The first step consists of adding a `driver subsystem` to the kernel. The corresponding code will
live in `src/driver.rs`. The subsystem introduces `interface::DeviceDriver`, a common trait that
every device driver will need to implement and that is known to the kernel. A global
`DRIVER_MANAGER` instance (of type `DriverManager`) that is instantiated in the same file serves as
the central entity that can be called to manage all things device drivers in the kernel. For
example, by using the globally accessible `crate::driver::driver_manager().register_driver(...)`,
any code can can register an object with static lifetime that implements the
`interface::DeviceDriver` trait.

During kernel init, a call to `crate::driver::driver_manager().init_drivers(...)` will let the
driver manager loop over all registered drivers and kick off their initialization, and also execute
an optional `post-init callback` that can be registered alongside the driver. For example, this
mechanism is used to switch over to the `UART` driver as the main system console after the `UART`
driver has been initialized.

## BSP Driver Implementation

In `src/bsp/raspberrypi/driver.rs`, the function `init()` takes care of registering the `UART` and
`GPIO` drivers. It is therefore important that during kernel init, the correct order of (i) first
initializing the BSP driver subsystem, and only then (ii) calling the `driver_manager()` is
followed, like the following excerpt from `main.rs` shows:

```rust
unsafe fn kernel_init() -> ! {
    // Initialize the BSP driver subsystem.
    if let Err(x) = bsp::driver::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::driver_manager().init_drivers();
    // println! is usable from here on.
```



The drivers themselves are stored in `src/bsp/device_driver`, and can be reused between `BSP`s. The
first driver added in these tutorials is the `PL011Uart` driver: It implements the
`console::interface::*` traits and is from now on used as the main system console. The second driver
is the `GPIO` driver, which pinmuxes (that is, routing signals from inside the `SoC` to actual HW
pins) the RPi's PL011 UART accordingly. Note how the `GPIO` driver differentiates between **RPi 3**
and **RPi 4**. Their HW is different, so we have to account for it in SW.

The `BSP`s now also contain a memory map in `src/bsp/raspberrypi/memory.rs`. It provides the
Raspberry's `MMIO` addresses which are used by the `BSP` to instantiate the respective device
drivers, so that the driver code knows where to find the device's registers in memory.

## Boot it from SD card

Since we have real `UART` output now, we can run the code on the real hardware. Building is
differentiated between the **RPi 3** and the **RPi 4** due to before mentioned differences in the
`GPIO` driver. By default, all `Makefile` targets will build for the **RPi 3**. In order to build
for the the **RPi 4**, prepend `BSP=rpi4` to each target. For example:

```console
$ BSP=rpi4 make
$ BSP=rpi4 make doc
```

Unfortunately, QEMU does not yet support the **RPi 4**, so `BSP=rpi4 make qemu` won't work.

**Some steps for preparing the SD card differ between RPi 3 and RPi 4, so be careful in the
following.**

### Common for both

1. Make a single `FAT32` partition named `boot`.
2. On the card, generate a file named `config.txt` with the following contents:

```txt
arm_64bit=1
init_uart_clock=48000000
```
### RPi 3

3. Copy the following files from the [Raspberry Pi firmware repo](https://github.com/raspberrypi/firmware/tree/master/boot) onto the SD card:
    - [bootcode.bin](https://github.com/raspberrypi/firmware/raw/master/boot/bootcode.bin)
    - [fixup.dat](https://github.com/raspberrypi/firmware/raw/master/boot/fixup.dat)
    - [start.elf](https://github.com/raspberrypi/firmware/raw/master/boot/start.elf)
4. Run `make`.

### RPi 4

3. Copy the following files from the [Raspberry Pi firmware repo](https://github.com/raspberrypi/firmware/tree/master/boot) onto the SD card:
    - [fixup4.dat](https://github.com/raspberrypi/firmware/raw/master/boot/fixup4.dat)
    - [start4.elf](https://github.com/raspberrypi/firmware/raw/master/boot/start4.elf)
    - [bcm2711-rpi-4-b.dtb](https://github.com/raspberrypi/firmware/raw/master/boot/bcm2711-rpi-4-b.dtb)
4. Run `BSP=rpi4 make`.


_**Note**: Should it not work on your RPi 4, try renaming `start4.elf` to `start.elf` (without the 4)
on the SD card._

### Common again

5. Copy the `kernel8.img` onto the SD card and insert it back into the RPi.
6. Run the `miniterm` target, which opens the UART device on the host:

```console
$ make miniterm
```

> ❗ **NOTE**: `Miniterm` assumes a default serial device name of `/dev/ttyUSB0`. Depending on your
> host operating system, the device name might differ. For example, on `macOS`, it might be
> something like `/dev/tty.usbserial-0001`. In this case, please give the name explicitly:


```console
$ DEV_SERIAL=/dev/tty.usbserial-0001 make miniterm
```

7. Connect the USB serial to your host PC.
    - Wiring diagram at [top-level README](../README.md#-usb-serial-output).
    - **NOTE**: TX (transmit) wire connects to the RX (receive) pin.
    - Make sure that you **DID NOT** connect the power pin of the USB serial. Only RX/TX and GND.
8. Connect the RPi to the (USB) power cable and observe the output:

```console
Miniterm 1.0

[MT] ⏳ Waiting for /dev/ttyUSB0
[MT] ✅ Serial connected
[0] mingo version 0.5.0
[1] Booting on: Raspberry Pi 3
[2] Drivers loaded:
      1. BCM PL011 UART
      2. BCM GPIO
[3] Chars written: 117
[4] Echoing input now
```

8. Exit by pressing <kbd>ctrl-c</kbd>.

