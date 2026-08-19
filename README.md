# Tutorial 02 - Runtime Init

## tl;dr

- We extend `boot.s` to call into Rust code for the first time. Before the jump
  to Rust happens, a bit of runtime init work is done.
- The firmware-provided device tree pointer is preserved across runtime setup and handed to Rust for
  later chapters to consume.
- An optional raw-GPIO LED trace can report boot stages and panic codes before a console exists.
- The Rust code being called just halts execution with a call to `panic!()`.
- Check out `make qemu` again to see the additional code run.

## Notable additions

- More additions to the linker script:
     - New sections: `.rodata`, `.got`, `.data`, `.bss`.
     - A dedicated place for linking boot-time arguments that need to be read by `_start()`.
- `_start()` in `_arch/__arch_name__/cpu/boot.s`:
     1. Halts core if core != core0.
     1. Saves the device tree pointer supplied by firmware in `x0`.
     1. Initializes the `DRAM` by zeroing the [bss] section.
     1. Sets up the `stack pointer`.
     1. Jumps to the `_start_rust()` function, defined in `arch/__arch_name__/cpu/boot.rs`.
- `_start_rust()`:
     - Receives the preserved firmware pointer and passes it to `kernel_init()`. Parsing the device
       tree is deliberately deferred until Chapter 20.
     - Calls `kernel_init()`, which calls `panic!()`, which eventually halts core0 as well.
- The `boot_trace` feature adds a minimal LED diagnostic path that does not depend on the later GPIO
  driver. It can mark early assembly/Rust stages and blink a numeric code on panic.
- The library now uses the [aarch64-cpu] crate, which provides zero-overhead abstractions and wraps
  `unsafe` parts when dealing with the CPU's resources.
    - See it in action in `_arch/__arch_name__/cpu.rs`.

[bss]: https://en.wikipedia.org/wiki/.bss
[aarch64-cpu]: https://github.com/rust-embedded/aarch64-cpu
