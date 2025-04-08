# Tutorial 07 - Timestamps

## tl;dr

- We add abstractions for timer hardware, and implement them for the ARM architectural timer in
  `_arch/aarch64`.
- The new timer functions are used to annotate UART prints with timestamps, and to get rid of the
  cycle-based delays in the `GPIO` device driver, which boosts accuracy.
- A `warn!()` macro is added.

## Test it

Check it out via chainboot (added in previous tutorial):
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
[MP] ⏩ Pushing 12 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.143123] mingo version 0.7.0
[    0.143323] Booting on: Raspberry Pi 3
[    0.143778] Architectural timer resolution: 52 ns
[    0.144352] Drivers loaded:
[    0.144688]       1. BCM PL011 UART
[    0.145110]       2. BCM GPIO
[W   0.145469] Spin duration smaller than architecturally supported, skipping
[    0.146313] Spinning for 1 second
[    1.146715] Spinning for 1 second
[    2.146938] Spinning for 1 second
```

