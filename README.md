# Tutorial 06 - UART Chainloader

## tl;dr

- Running from an SD card was a nice experience, but it would be extremely tedious to do it for
  every new binary. So let's write a [chainloader].
- This will be the last binary you need to put on the SD card. Each following tutorial will provide
  a `chainboot` target in the `Makefile` that lets you conveniently load the kernel over `UART`.

[chainloader]: https://en.wikipedia.org/wiki/Chain_loading


## Note

Please note that there is stuff going on in this tutorial that is very hard to grasp by only looking
at the source code changes.

The gist of it is that in `boot.s`, we are writing a piece of [position independent code] which
automatically determines where the firmware has loaded the binary (`0x8_0000`), and where it was
linked to (`0x200_0000`, see `kernel.ld`). The binary then copies itself from loaded to linked
address (aka  "relocating" itself), and then jumps to the relocated version of `_start_rust()`.

Since the chainloader has put itself "out of the way" now, it can now receive another kernel binary
from the `UART` and copy it to the standard load address of the RPi firmware at `0x8_0000`. Finally,
it jumps to `0x8_0000` and the newly loaded binary transparently executes as if it had been loaded
from SD card all along.

Please bear with me until I find the time to write it all down here elaborately. For the time being,
please see this tutorial as an enabler for a convenience feature that allows booting the following
tutorials in a quick manner. _For those keen to get a deeper understanding, it could make sense to
skip forward to [Chapter 15](../15_virtual_mem_part3_precomputed_tables) and read the first half of
the README, where `Load Address != Link Address` is discussed_.

[position independent code]: https://en.wikipedia.org/wiki/Position-independent_code

## Install and test it

Our chainloader is called `MiniLoad` and is inspired by [raspbootin].

You can try it with this tutorial already:
1. Depending on your target hardware, run:`make` or `BSP=rpi4 make`.
1. Copy `kernel8.img` to the SD card and put the SD card back into your RPi.
1. Run `make chainboot` or `BSP=rpi4 make chainboot`.
1. Connect the USB serial to your host PC.
    - Wiring diagram at [top-level README](../README.md#-usb-serial-output).
    - Make sure that you **DID NOT** connect the power pin of the USB serial. Only RX/TX and GND.
1. Connect the RPi to the (USB) power cable.
1. Observe the loader fetching a kernel over `UART`:

> ❗ **NOTE**: `make chainboot` assumes a default serial device name of `/dev/ttyUSB0`. Depending on
> your host operating system, the device name might differ. For example, on `macOS`, it might be
> something like `/dev/tty.usbserial-0001`. In this case, please give the name explicitly:


```console
$ DEV_SERIAL=/dev/tty.usbserial-0001 make chainboot
```

[raspbootin]: https://github.com/mrvn/raspbootin

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
[MP] ⏩ Pushing 7 KiB ==========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[0] mingo version 0.5.0
[1] Booting on: Raspberry Pi 3
[2] Drivers loaded:
      1. BCM PL011 UART
      2. BCM GPIO
[3] Chars written: 117
[4] Echoing input now
```

In this tutorial, a version of the kernel from the previous tutorial is loaded for demo purposes. In
subsequent tutorials, it will be the working directory's kernel.

## Test it

The `Makefile` in this tutorial has an additional target, `qemuasm`, that lets you nicely observe
how the kernel, after relocating itself, jumps the load address region (`0x80_XXX`) to the relocated
code at (`0x0200_0XXX`):

```console
$ make qemuasm
[...]
N:
0x00080030:  58000140  ldr      x0, #0x80058
0x00080034:  9100001f  mov      sp, x0
0x00080038:  58000141  ldr      x1, #0x80060
0x0008003c:  d61f0020  br       x1

----------------
IN:
0x02000070:  9400044c  bl       #0x20011a0

----------------
IN:
0x020011a0:  90000008  adrp     x8, #0x2001000
0x020011a4:  90000009  adrp     x9, #0x2001000
0x020011a8:  f9446508  ldr      x8, [x8, #0x8c8]
0x020011ac:  f9446929  ldr      x9, [x9, #0x8d0]
0x020011b0:  eb08013f  cmp      x9, x8
0x020011b4:  54000109  b.ls     #0x20011d4
[...]
```

