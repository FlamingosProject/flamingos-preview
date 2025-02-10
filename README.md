# Tutorial 03 - Hacky Hello World

## tl;dr

- Introducing global `println!()` macros to enable "printf debugging" at the earliest.
- To keep tutorial length reasonable, printing functions for now "abuse" a QEMU property that lets
  us use the Raspberry's `UART` without setting it up properly.
- Using the real hardware `UART` is enabled step-by-step in following tutorials.

## Notable additions

- `src/console.rs` introduces interface `Traits` for console commands and global access to the
  kernel's console through `console::console()`.
- `src/bsp/raspberrypi/console.rs` implements the interface for QEMU's emulated UART.
- The panic handler makes use of the new `println!()` to display user error messages.
- There is a new Makefile target, `make test`, intended for automated testing. It boots the compiled
  kernel in `QEMU`, and checks for an expected output string produced by the kernel.
  - In this tutorial, it checks for the string `Stopping here`, which is emitted by the `panic!()`
    at the end of `main.rs`.

## Test it

QEMU is no longer running in assembly mode. It will from now on show the output of the `console`.

```console
$ make qemu
[...]

Hello from Rust!
Kernel panic!

Panic location:
      File 'src/main.rs', line 126, column 5

Stopping here.
```
