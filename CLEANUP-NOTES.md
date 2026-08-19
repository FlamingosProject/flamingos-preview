* Clean up Makefile stuff and mostly replace with Cargo
* 01: Does not halt processor, just WFE loops. Explain
  what displayed ASM is, explain multicore
* Lots of machinery that isn't explained well
* Whole thing with optional qemu docker build but rest local
* qemu renamed raspi3 to raspi3b at some point
* 03: `PANIC_IN_PROGRESS` should be handled with
  `.fetch_or(true, AcqRel)` to avoid worries about
  atomicity
* Restructure everything so that there is a branch per
  chapter properly stacked, allowing rebasing to correct
  code in current and subsequent chapters. Some scripts
  would be helpful here
* Clean up crlf mess starting ch 5.
* Replace the Ruby terminal with Rust `scip` plus a Rust
  chainloader host replacement
* Add a crc32 to the chainloader to validate the kernel
