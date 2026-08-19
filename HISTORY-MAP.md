# Curated History Mapping

This report records the August 19, 2026 rewrite described in
[CLEANUP-PLAN.md](CLEANUP-PLAN.md). The archived refs preserve the actual development history; the
new numbered refs present the tutorial as one linear chain with one commit per chapter.

The `main` row maps the archived display-tree tip to the final chapter-addition commit. The public
`main` tip is its immediate child, which adds this report. A commit cannot contain its own object ID,
so the report commit is identified by `git rev-parse main` rather than embedded below.

## Ref Mapping

| Ref | Archived commit | Curated commit | Archived tree | Curated tree | Tree check |
| --- | --- | --- | --- | --- | --- |
| `main` | `771a2571e23267ea3d882d917a462b4f19c6f2d9` | `9a77605f0cd7f201e8015f46b13c388d79b168a1` | `da5f2472da6f077b1e9c46b234a99341397af350` | `d05af107394ef3acb2f21bc9f223d2ad2abec1ad` | Intentional reconstruction |
| `01-wait-forever` | `1a7650bba8fab5da2385c6475a6e7a83496f05ad` | `187aacb125585bd563e01794b202c922d35fe5f4` | `6b5f5b3cc77f6142baf37b15a789eb06cd10b757` | `6b5f5b3cc77f6142baf37b15a789eb06cd10b757` | PASS |
| `02-runtime-init` | `3c96ca7bf1a0783252eb2cf57c463e1398244362` | `63fa58a9ada42d7d8db4bf48e3e2f05b30b7b6f9` | `fb47d7e4c56741172b37ef5dc06e4c5e69b5823e` | `fb47d7e4c56741172b37ef5dc06e4c5e69b5823e` | PASS |
| `03-hello-world` | `c2265b385551daa12a80d31aff81ff258b758655` | `13dfc7e2475b2bfcd4ec6ee7f1e4f2ed24a75d1e` | `2ceb805ec0794e3fa26f143c3f2e935fdb9f512b` | `2ceb805ec0794e3fa26f143c3f2e935fdb9f512b` | PASS |
| `04-safe-globals` | `516294106f5da7c7a05e0c961983ad59b468156e` | `7402d0d6746353ff1bbedfb70da5350dddd8bffa` | `a6607a0395b8a5bdf54e1046e4099d0e0bd37874` | `a6607a0395b8a5bdf54e1046e4099d0e0bd37874` | PASS |
| `05-drivers-gpio-uart` | `1d9dee40c140859835615a164d386ac670bfdc2f` | `92b6e8bf2addc5380e3ce1aa69234c0fafc0f4f9` | `72cc79339ab402b3fe0754ffd68d8c042e1d415c` | `72cc79339ab402b3fe0754ffd68d8c042e1d415c` | PASS |
| `06-uart-chainloader` | `721c72f0a0bb2c38c49f21e5cbd8d70834a6938a` | `8d75ee9734734655dee2185f55ba23812336c309` | `0cdc9dceb1f1869b02843430db914b40f499bdcb` | `0cdc9dceb1f1869b02843430db914b40f499bdcb` | PASS |
| `07-timestamps` | `2d0890678e892920a8a7b7efc84fad035ceaad6b` | `bb3d4b39aeda770bcd17d5b960f343d2c9de818f` | `c014e0011f8508b0f538091fcea53f3b9fcec58f` | `c014e0011f8508b0f538091fcea53f3b9fcec58f` | PASS |
| `08-hw-debug-jtag` | `e38aa2a14dd4193f9726b94f06b88ccf67e6c2e7` | `eab555d8b123b04f22d7ae5d8309a9d9f3eacc42` | `322c933f02c5b5d669029dff46d1fb523851b77c` | `322c933f02c5b5d669029dff46d1fb523851b77c` | PASS |
| `09-privilege-level` | `e9c4979f4b4c908aa9126af15a72f48c061ab145` | `a61b350a5e6a5b69d2a641a1920e923bafa37db3` | `3c99184ee54acf905d7d9fea3e5ea75f6753fa52` | `3c99184ee54acf905d7d9fea3e5ea75f6753fa52` | PASS |
| `10-virtual-mem-part1` | `2a4cb87a3255681df722a688c3ab8ccae7a36b33` | `6e43ab841b78645193bbd6bc01d6890f647a819c` | `d9f6b4d7c42a4bc3a6aeac3e894d129f4b51dd50` | `d9f6b4d7c42a4bc3a6aeac3e894d129f4b51dd50` | PASS |
| `11-exceptions-part1` | `89fb31cff9eb13a7dcc53de1ab247fe7ccf5e5b8` | `ec3d9488bf5ce3158e4645c76548b89e3a5c411f` | `58e6d1231ce38e3e6e3989648557aaa48a055670` | `58e6d1231ce38e3e6e3989648557aaa48a055670` | PASS |
| `12-integrated-testing` | `49a5f8c53d3fecca38db61282e29f86945db3247` | `aa903a951d5819530697340e6621ad3293b6a491` | `006ac4da1993a04973419085baefe2747c4c0f64` | `006ac4da1993a04973419085baefe2747c4c0f64` | PASS |
| `13-exceptions-part2` | `9004e693e55ca493bee54cb04c540c088c4e3f11` | `ecd4be6c959d318e2103aef253500d570c475d00` | `af7a3894be56f04cd8702842fcbd5c7dc055171b` | `af7a3894be56f04cd8702842fcbd5c7dc055171b` | PASS |
| `14-virtual-mem-part2` | `468bc1465a922ea2729dfcc779e549c979b355c9` | `c793b5c88619f57803f09e159574debe1b8f804b` | `2a8172258a9bd2dfa7312b4f26b935468ab2d7c7` | `2a8172258a9bd2dfa7312b4f26b935468ab2d7c7` | PASS |
| `15-virtual-mem-part3` | `50dab25a3927cfd83919be8613c11a849e104feb` | `e91574c1946c1829ef4b50231d134728d9286569` | `32123539c2657f9f7dbd4205c12544a89daf3c11` | `32123539c2657f9f7dbd4205c12544a89daf3c11` | PASS |
| `16-virtual-mem-part4` | `eb1f11dc39f90ac3edd2ccca40e9885675c3e656` | `b635286af3bea918510f1e1ebf93715873b5bdc0` | `13fe70187ab580d640fab82bd19f65912a059c28` | `13fe70187ab580d640fab82bd19f65912a059c28` | PASS |
| `17-kernel-symbols` | `6ee15800f9f2196eac5e943e10170829e2e2a857` | `a6e97fe4ea75d78f345939de1e406eb1c676d238` | `0b9cf59c344f44e31e93e37ac2c0c6359320b369` | `0b9cf59c344f44e31e93e37ac2c0c6359320b369` | PASS |
| `18-kernel-heap` | `21155decc6a8a1b58ba6683e57dc2619380f7d23` | `542af98367574a551bc97a1763f3889121ab0fdb` | `65205852e30d4d2471210d47c02ea205f569bfff` | `65205852e30d4d2471210d47c02ea205f569bfff` | PASS |
| `19-timer-callbacks` | `a9fac47d82ce4119b049e32b0d016bde9622a46f` | `a93ac20f34345017aa29740fc20f8d9e45f7bee3` | `fd06d6ed7e71c52ed72d60d795796ef90c0f0a26` | `fd06d6ed7e71c52ed72d60d795796ef90c0f0a26` | PASS |
| `20-boot-improvements` | `efe935bc6338784483c965d3039766153207ff38` | `30b3d001dbcab7eae4df8710623bea016e3aed5e` | `d40cb16f20cba36bf86703d172350d0f11e3f093` | `d40cb16f20cba36bf86703d172350d0f11e3f093` | PASS |
| `21-second-core` | `c3c508b71ea18a37a097ca68eea1de97f8c4d873` | `3633947e2471dec647168e67f8679ebdd130fcb5` | `4871ece1bf56354b73ea51780394c7d29949b94e` | `4871ece1bf56354b73ea51780394c7d29949b94e` | PASS |

## Validation

All 21 curated chapter trees exactly match their archived canonical trees. The branch chain contains
exactly 21 post-boundary commits, and every chapter tip is the direct parent of the next chapter's
single commit.

The validation matrix recorded 227 passing checks. It covered both Raspberry Pi BSP builds for
every chapter, early QEMU smoke or boot tests, stable integration tests from Chapter 12 onward,
chainloader builds from Chapter 06 onward, JTAG preparation from Chapter 08 onward, kernel Clippy,
and native host-tool Clippy.

Thirty-three audit-only failures were retained rather than changing canonical chapter content:

- Current stable `rustfmt` would rewrite 16 archived trees.
- The Chapter 12-and-later aggregate `make clippy` target incorrectly applies the bare-metal target
  to native workspace tools. Kernel and host tools pass when checked under their intended targets.
- Native tools added from Chapter 15 do not satisfy the kernel-only `-D missing_docs` policy. They
  pass ordinary Clippy under the native target.

The archived history is reachable at `archive/2026-08-19/*`. Independent recovery copies also exist
at `/usr/local/src/fp/flamingos-preview-pre-cleanup-2026-08-19.bundle` and
`/usr/local/src/fp/flamingos-preview.bak2` at the time of the rewrite.
