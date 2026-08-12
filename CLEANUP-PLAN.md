# Repository History Cleanup Plan

## Purpose

The repository history since the Flamingos fork began in November 2024 records useful work, but it
does not present that work in the form the project uses today. Early fork development modified a
monorepo containing one directory per chapter. Later development replaced those directories with a
top-level display tree of submodules, while the chapter implementations moved onto numbered
branches. Subsequent fixes, backports, branch construction, and submodule-pointer updates produced a
history that is difficult to follow.

We propose replacing the public post-fork history with two deliberately different artifacts:

1. An immutable archive of the actual development history.
2. A curated history that presents the tutorial as a linear sequence with one coherent commit per
   chapter, plus a separate clean history for constructing the top-level display tree.

This is a presentation rewrite, not an attempt to erase the historical record. The archive remains
the authoritative account of how the work actually happened. The curated history becomes the
primary interface for readers and future development.

## Proposed Fork Boundary

The likely boundary is commit `644474cc`, the final upstream commit before Bart's first fork-era
commit, `e9bd4cc5`, in November 2024. All commits through `644474cc` would remain unchanged and would
be shared ancestry for the archived and curated histories.

We will verify and explicitly approve this boundary before changing any refs.

## Resulting Repository Shape

The rewrite creates three related structures.

### Preserved Upstream History

The complete upstream history through the fork boundary remains unchanged:

```text
upstream commits -- 644474cc
```

No upstream commit is rewritten.

### Archived Fork History

The existing post-fork history continues from the same boundary under archival refs:

```text
upstream commits -- 644474cc -- actual 2024-2026 development history
                                      |
                                      +-- archive refs
```

This preserves the original directory-per-chapter work, restructuring experiments, creation of
chapter branches, conversion to submodules, pointer updates, fixes, backports, and all intermediate
commits.

The archive is not placed in the ancestry of the new `main`. Doing that would retain the tangled
post-fork history in ordinary `git log` output and defeat the purpose of the cleanup. Instead, the
archived and curated histories are alternative continuations from `644474cc`.

### Curated Chapter History

The numbered chapter branches form one linear development chain:

```text
644474cc--C01--C02--C03--...--C20--C21
            |    |                    |
            |    |                    +-- 21-second-core
            |    +-- 02-runtime-init
            +-- 01-wait-forever
```

Each `Cnn` commit changes the previous chapter into the next chapter. Each numbered branch points to
its corresponding commit. The final tree at every branch tip is byte-for-byte identical to the
current canonical tree for that chapter.

The intended invariant is exactly one new commit between adjacent chapter tips. Chapter 12, which
currently contains two chapter-specific commits, will be folded into one coherent Chapter 12 commit.

### Curated Display-Tree History

The top-level `main` branch is a separate line that constructs the repository's display tree:

```text
644474cc--V0--V01--V02--...--V20--V21  main
                 |    |          |
                 |    |          +---- gitlink to C21
                 |    +--------------- gitlink to C02
                 +-------------------- gitlink to C01
```

`V0` converts the old top-level layout into the clean aggregate scaffolding. Each subsequent display
commit adds one numbered chapter submodule and any aggregate documentation that logically belongs
with that addition. The final `main` tree contains all chapter submodules and the current top-level
documentation and configuration.

The arrows from display-tree commits to chapter commits are submodule gitlinks, not commit-parent
relationships. The chapter development chain and display-tree construction therefore remain
separate, comprehensible histories.

## Why We Will Reconstruct Snapshots

We will not mechanically rebase the old monorepo commits onto the numbered branches. A historical
commit can modify several chapter directories, root tooling, CI, documentation, and temporary
infrastructure at once. There is no unique or useful way to assign such a commit to one chapter
branch.

Instead, the current numbered branch tips are canonical snapshots. For each chapter, we will create
one commit whose resulting tree exactly matches that snapshot. This preserves the tutorial content
while rationalizing its presentation.

The archive will remain available whenever the chronological development process, individual
experiments, or exact original attribution matters.

## Historical Preservation

Before rewriting any public ref, we will preserve the current repository in three ways.

### Remote Archive Refs

We will create immutable remote refs for `main` and every numbered branch, using a dated namespace
such as:

```text
archive-2026-08-12/main
archive-2026-08-12/01-wait-forever
archive-2026-08-12/02-runtime-init
...
archive-2026-08-12/21-second-core
```

Archiving `main` alone is insufficient. A submodule gitlink records another commit's object ID but
does not make that commit reachable through commit ancestry. Every chapter tip therefore needs its
own archive ref, or an equivalent explicit reachability anchor.

### External Bundle

We will create and verify a complete Git bundle containing all refs before the rewrite. The bundle
will be stored outside the working repository and will provide an independent recovery path even if
remote archive refs are changed accidentally.

### Old-to-New Mapping

We will produce a mapping table that records, for every rewritten ref:

- its archived commit ID;
- its rewritten commit ID;
- its archived tree ID;
- its rewritten tree ID; and
- the result of the tree-equality check.

This gives reviewers a concrete way to verify that the history changed while chapter contents did
not.

## Authorship Convention

Bart Massey and Philipp Oppermann performed the fork-era work jointly. Attribution at the curated
commit level does not need to reproduce the authorship granularity of the archived history.

Every human-authored curated commit will list both Bart and Philipp, using one consistent primary
author and a `Co-authored-by` trailer for the other. We will agree on the primary-author convention
before constructing commits. The archive remains the source of exact original authorship and dates.

Generated or purely mechanical commits, if any are unavoidable, will be identified explicitly.

## Commit Content and Messages

Each chapter commit should explain the conceptual development introduced by that chapter, not the
mechanics of the history rewrite. The chapter summaries and branch-to-branch diffs will be used to
draft accurate subjects and bodies.

Each display-tree commit should explain which chapter becomes visible in the aggregate checkout and
any top-level documentation or configuration added with it.

We will review the complete commit-message and authorship plan before creating the final history.

## Validation Requirements

Validation is part of construction, not a final cleanup step.

For every numbered chapter:

- the rewritten tree ID must equal the archived canonical tree ID;
- its branch must descend directly from the preceding chapter tip;
- exactly one commit must separate adjacent chapter tips;
- repository formatting and structural checks must pass; and
- the build and test coverage appropriate to that chapter must pass.

For the complete chapter chain:

- all numbered branches must lie on one linear history;
- persistent features must remain present from their chapter of introduction onward;
- submodule URLs and branch metadata must be consistent; and
- no Ruby, obsolete test infrastructure, or other intentionally removed material may reappear.

For the display tree:

- every gitlink must equal the corresponding rewritten branch tip;
- every submodule checkout must be clean;
- the final aggregate tree must contain the intended current files; and
- cloning and recursively initializing the repository must produce the expected display tree.

The rewrite will be built and tested in an isolated worktree. Existing public refs will remain
unchanged until review is complete.

## Execution Sequence

1. Confirm `644474cc` as the fork boundary.
2. Freeze a list of current refs, commits, trees, and submodule pointers.
3. Create and push the remote archive refs.
4. Create, verify, and externally store a complete Git bundle.
5. Draft and review the 21 chapter commit messages and authorship metadata.
6. Reconstruct the chapter chain from canonical snapshots in an isolated worktree.
7. Validate each chapter immediately after creating it.
8. Verify exact tree equality between every archived and rewritten chapter tip.
9. Construct the clean display-tree history and point it at the rewritten chapter tips.
10. Produce and review the old-to-new commit and tree mapping.
11. Review the final graphs, commit messages, archive refs, and validation results.
12. Force-update public chapter branches and `main` only after explicit approval.
13. Verify all remote refs after the push and retain the archive refs and bundle permanently.

## Review Decisions

Before implementation, Bart and Philipp should agree on:

1. Whether `644474cc` is the correct fork boundary.
2. The dated archive-ref namespace and retention policy.
3. Where the verified external bundle will be stored.
4. Which person is the primary author on curated commits and the exact co-author identities.
5. Whether the display tree should use one setup commit plus one commit per chapter, as proposed, or
   a smaller number of aggregate commits.
6. The desired depth of chapter-by-chapter build and runtime validation.
7. Whether the mapping report belongs in the final repository or only in the archival material.

No history rewrite should begin until these decisions and the preservation artifacts have been
reviewed.
