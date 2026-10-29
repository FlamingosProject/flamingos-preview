# Reviewing the Curated Tutorial History

This preview presents the Flamingos Raspberry Pi OS tutorial as
two related histories:

- `main` is a display tree whose numbered directories are submodules.
- The numbered branches form the tutorial itself. Chapters 01–21
  each add one commit, starting from the preserved pre-fork history
  at `644474cc`. Chapters beyond 21 may retain multiple development
  commits and can be squashed later.

Clone the display tree with:

```console
git clone --recurse-submodules https://github.com/FlamingosProject/flamingos-preview.git
```

## Suggested Review

The most useful review is of the progression between numbered
branches. To see the complete curated sequence:

```console
git log --reverse --oneline 644474cc..refs/heads/21-second-core
```

To inspect one chapter's conceptual change, show its single
commit or compare adjacent branches:

```console
git show refs/heads/13-exceptions-part2
git diff refs/heads/12-integrated-testing..refs/heads/13-exceptions-part2
```

Please focus on whether each change appears in the right chapter,
whether chapter-to-chapter development is understandable, and
whether later chapters preserve previously introduced
capabilities. The chapter READMEs should describe the code and
workflows available at their branch tips.

## History Preservation

This is a presentation rewrite, not a deletion of the actual
development history. [CLEANUP-PLAN.md](CLEANUP-PLAN.md) describes
the policy and [HISTORY-MAP.md](HISTORY-MAP.md) records the
old-to-new mapping and validation results. Original development
refs are under `archive/2026-08-19/`; the first curated version
is under `archive/2026-08-19-curated-v1/`.

The October 29 maintenance folded chapter follow-up fixes into
the 21 curated commits and reparented Chapters 22–23. Its
previous tips are preserved locally under
`archive/2026-10-29-pre-recuration/`. The mapping report
distinguishes historical and current revisions. The user has
published the rewritten chapter tips; local and recorded `origin`
chapter refs now agree. `STATUS.md` records the current handoff.

## Validation Boundary

The original cleanup matrix completed 228 checks across all
chapter tips, including history and change-scope audits, stable
formatting, Raspberry Pi 3 and 4 builds and Clippy targets,
applicable QEMU boot and integration tests, chainloader builds,
and JTAG preparation. A later board-selection audit verified all
three BSP symbols at every tip, with clean Raspberry Pi Zero 2 W
builds and Clippy checks plus applicable tests, chainloader
builds, and JTAG configuration selection.

The October maintenance checks and current hardware boundary are
recorded in [STATUS.md](STATUS.md). The older matrix above is
historical evidence, not a fresh run against every rewritten tip.

OpenOCD/JTAG/GDB attachment and debugging still require physical
hardware validation. The later storage handoff records successful
Zero 2 W reads and use of the UART chainboot workflow; it does
not establish every board's handoff or storage behavior.

## CI Scope

Chapter workflows run on pushes and pull requests without a
branch name filter. They live at the repository root when a
chapter branch is checked out; nested submodule workflows do not
run on `main`.

- Chapters 01–02 build both hardware families.
- Chapters 03–11 also run their stable QEMU boot test.
- Chapters 12–23 run stable QEMU boot and integration tests.

The inherited Ruby sanity/unit/extra-test jobs were removed from
early chapters, and their build workflows use stable Rust setup.
Chapter 23 also runs host ELF-loader tests. They generate and
remove their executable on an in-memory exFAT card, without
requiring a prebuilt ELF or physical media.
