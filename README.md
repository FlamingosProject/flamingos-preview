# Tutorial 03 - Hacky Hello World

## tl;dr

- Introducing global `println!()` macros to enable "printf debugging" at the earliest.
- Introducing an untimestamped `warn!()` severity alongside the basic print macros.
- To keep tutorial length reasonable, printing functions for now "abuse" a QEMU property that lets
  us use the Raspberry's `UART` without setting it up properly.
- Using the real hardware `UART` is enabled step-by-step in following tutorials.

## Notable additions

- `src/console.rs` introduces interface `Traits` for console commands and global access to the
  kernel's console through `console::console()`.
- `src/bsp/raspberrypi/console.rs` implements the interface for QEMU's emulated UART.
- The panic handler makes use of the new `println!()` to display user error messages.
- `make test_boot` builds a separate `test_build` kernel and boots it with QEMU semihosting enabled.
  Reaching the boot checkpoint explicitly exits QEMU successfully, while a panic exits with failure.
  This makes the boot smoke test deterministic without matching normal console output.

## Test it

QEMU is no longer running in assembly mode. A normal `make qemu` run will from now on show the output
of the `console` and end at the tutorial's deliberate panic:

```console
$ make qemu
[...]

Hello from Rust!
Kernel panic!

Panic location:
      File 'src/main.rs', line 126, column 5

Stopping here.
```
