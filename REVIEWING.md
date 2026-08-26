# Reviewing the Curated Tutorial History

This preview presents the Flamingos Raspberry Pi OS tutorial as two related histories:

- `main` is a display tree whose numbered directories are submodules.
- The numbered branches form the tutorial itself. Each branch is one commit beyond the preceding
  chapter, starting from the preserved pre-fork history at `644474cc`.

Clone the display tree with:

```console
git clone --recurse-submodules https://github.com/FlamingosProject/flamingos-preview.git
```

## Suggested Review

The most useful review is of the progression between numbered branches. To see the complete curated
sequence:

```console
git log --reverse --oneline 644474cc..refs/remotes/origin/21-second-core
```

To inspect one chapter's conceptual change, show its single commit or compare adjacent branches:

```console
git show refs/remotes/origin/13-exceptions-part2
git diff refs/remotes/origin/12-integrated-testing..refs/remotes/origin/13-exceptions-part2
```

Please focus on whether each change appears in the right chapter, whether chapter-to-chapter
development is understandable, and whether later chapters preserve previously introduced
capabilities. The chapter READMEs should describe the code and workflows available at their branch
tips.

## History Preservation

This is a presentation rewrite, not a deletion of the actual development history.
[CLEANUP-PLAN.md](CLEANUP-PLAN.md) describes the policy and
[HISTORY-MAP.md](HISTORY-MAP.md) records the old-to-new mapping and validation results. Original
development refs are under `archive/2026-08-19/`; the first curated version is under
`archive/2026-08-19-curated-v1/`.

## Validation Boundary

The original cleanup matrix completed 228 checks across all chapter tips, including history and
change-scope audits, stable formatting, Raspberry Pi 3 and 4 builds and Clippy targets, applicable
QEMU boot and integration tests, chainloader builds, and JTAG preparation. A later board-selection
audit verified all three BSP symbols at every tip, with clean Raspberry Pi Zero 2 W builds and
Clippy checks plus applicable tests, chainloader builds, and JTAG configuration selection.

Two workflows still require end-to-end validation on physical hardware:

- UART chainloading and kernel handoff on a Raspberry Pi.
- OpenOCD/JTAG/GDB attachment and debugging on a Raspberry Pi.
