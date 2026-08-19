# Flamingos Raspberry Pi OS Tutorials

This repository presents a sequence of freestanding Rust kernels for the Raspberry Pi 3 and
Raspberry Pi 4. Each numbered directory is a Git submodule pinned to the corresponding numbered
branch. The branch tips form a single linear history: each chapter is exactly one commit beyond the
previous chapter.

Clone the complete display tree with:

```console
git clone --recurse-submodules https://github.com/FlamingosProject/flamingos-preview.git
```

To initialize an existing checkout:

```console
git submodule update --init --recursive
```

Each chapter is independently buildable. Enter its directory and use `make` with `BSP=rpi3` or
`BSP=rpi4`; the chapter README describes its concepts and the workflows available at that stage.

The public history was curated in August 2026 so that the tutorial progression is straightforward
to inspect. [CLEANUP-PLAN.md](CLEANUP-PLAN.md) explains the preservation and reconstruction policy.
The old development history remains available under `archive/2026-08-19/*` refs.

## License

This project is distributed under either the Apache License, Version 2.0, or the MIT License, at
your option.
