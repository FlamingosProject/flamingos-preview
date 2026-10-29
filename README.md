# Flamingos Raspberry Pi OS Tutorials

Flamingos is a hands-on bare-metal operating system tutorial for
64-bit Arm Raspberry Pis. Its lineage begins with [Zoltan
Baldaszti's bare-metal Raspberry Pi 3 tutorial in
C](https://github.com/bztsrc/raspi3-tutorial), which [Andre
Richter](https://github.com/andre-richter) took as a baseline,
adapted into Rust and extended on the content and capabilities.
Flamingos grew from Andre's Rust Raspberry Pi OS tutorials
through joint work by Philipp Oppermann and Bart Massey, with an
emphasis on stable Rust, practical hardware bring-up, and
development tools that remain useful as the kernel becomes more
capable.

Rather than presenting only a finished kernel, Flamingos keeps
every intermediate state as a first-class artifact. The sequence
begins with the smallest bootable image and develops console and
driver support, virtual memory, exceptions, interrupts, symbolic
backtraces, heap allocation, and multicore startup while keeping
the low-level mechanics visible. Each numbered directory is a Git
submodule pinned to its chapter branch. Chapters 01–21 form a
curated chain with exactly one commit per chapter. Chapters beyond
21 may have multiple commits as new work develops; they can be
squashed later. Chapters 22–23 currently extend the chain with
storage and executable loading.

Clone the complete display tree with:

```console
git clone --recurse-submodules https://github.com/FlamingosProject/flamingos-preview.git
```

To initialize an existing checkout:

```console
git submodule update --init --recursive
```

Each chapter is independently buildable for Raspberry Pi Zero 2 W
(`BSP=rpiz2`), Raspberry Pi 3 (`BSP=rpi3`), and Raspberry Pi 4
(`BSP=rpi4`). The Zero 2 W is the project's primary physical
target and uses the compatible RPi 3 kernel configuration. Enter
a chapter directory and run, for example, `BSP=rpiz2 make`; the
chapter README describes its concepts and available workflows.

The public history was curated in August 2026 so that the
tutorial progression is straightforward to inspect.
[REVIEWING.md](REVIEWING.md) explains how to examine the chapter
sequence and the current validation boundary.
[CLEANUP-PLAN.md](CLEANUP-PLAN.md) explains the preservation and
reconstruction policy. The old development history remains
available under `archive/2026-08-19/*` refs.

[STATUS.md](STATUS.md) is the project-wide handoff: the active
chapter, recent validation, and the next task. It is maintained
in the display tree as milestones change. Chapter READMEs
describe the behavior of their individual snapshots.

## License

This project is distributed under either the Apache License,
Version 2.0, or the MIT License, at your option.
