* Clean up Makefile stuff and mostly replace with Cargo
* 01: Does not halt processor, just WFE loops. Explain
  what displayed ASM is, explain multicore
* Lots of machinery that isn't explained well
* Whole thing with optional qemu docker build but rest local
* qemu renamed raspi3 to raspi3b at some point
* 03: `PANIC_IN_PROGRESS` should be handled with
  `.fetch_or(true, AcqRel)` to avoid worries about
  atomicity
