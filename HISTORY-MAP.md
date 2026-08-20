# Curated History Mapping

This report records the August 19, 2026 rewrite described in
[CLEANUP-PLAN.md](CLEANUP-PLAN.md) and its subsequent stable-tooling cleanup. The original archive
preserves the actual development history, while the numbered refs present the tutorial as one
linear chain with one commit per chapter.

The `main` rows map display-tree content tips. The public `main` tip is their immediate maintenance
child, which adds this report. A commit cannot contain its own object ID, so the report commit is
identified by `git rev-parse main` rather than embedded below.

## Original Rewrite Mapping

The first curated version was content-identical to the frozen chapter snapshots and remains under
`archive/2026-08-19-curated-v1/*`.

| Ref | Archived commit | Curated v1 commit | Archived tree | Curated v1 tree | Tree check |
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

## Current Curated Mapping

The current version includes the stable-tooling cleanup, Raspberry Pi Zero 2 W board selection, and
the isolated symbol-test target-directory fix. The scope check confirms that every tree difference
from curated v1 is confined to formatter output, `Makefile`, `kernel_symbols.mk` from Chapter 17
onward, and the associated README documentation.

| Ref | Curated v1 commit | Current commit | Curated v1 tree | Current tree | Scope check |
| --- | --- | --- | --- | --- | --- |
| `main` | `b7f8a56b28233f2d8e1db3266f744b5d39a61c14` | `3f075d54b9862483fa15aa737d9412c3926c9507` | `e27d29809796d64db0ca762d3f590c17d473d26d` | `89a01554c9c42ff88051258c009d4134a3251f88` | Display reconstruction |
| `01-wait-forever` | `187aacb125585bd563e01794b202c922d35fe5f4` | `2df84e1b87db9bb41c15163cc939f72be3dd5ee5` | `6b5f5b3cc77f6142baf37b15a789eb06cd10b757` | `4360c4b075dcf3e62d132a6f87cbd814da4bc41d` | PASS |
| `02-runtime-init` | `63fa58a9ada42d7d8db4bf48e3e2f05b30b7b6f9` | `cbe698ca3fefe3c1687b100fdfbc993038fa5547` | `fb47d7e4c56741172b37ef5dc06e4c5e69b5823e` | `8e26600baeae91940fd04500b143d897e95659c0` | PASS |
| `03-hello-world` | `13dfc7e2475b2bfcd4ec6ee7f1e4f2ed24a75d1e` | `f226a3c444e458119693a94bb02a029b9301bf1b` | `2ceb805ec0794e3fa26f143c3f2e935fdb9f512b` | `d1aa3c708963e7312fa255fcc242d8273a9966dd` | PASS |
| `04-safe-globals` | `7402d0d6746353ff1bbedfb70da5350dddd8bffa` | `f005f97f97a814f8bdd96395c0e072ca47a4b787` | `a6607a0395b8a5bdf54e1046e4099d0e0bd37874` | `8443750982f621f485af6e8dc8bf655ec5a1ce57` | PASS |
| `05-drivers-gpio-uart` | `92b6e8bf2addc5380e3ce1aa69234c0fafc0f4f9` | `67f3c235ce6dc877d4c0fad934d802f9df783161` | `72cc79339ab402b3fe0754ffd68d8c042e1d415c` | `c77130b47524bc18cfbc48f53cecb198423f6da2` | PASS |
| `06-uart-chainloader` | `8d75ee9734734655dee2185f55ba23812336c309` | `5e7ccac803569ef0510afb66823a1b7572c13426` | `0cdc9dceb1f1869b02843430db914b40f499bdcb` | `487314340e6e0aff8c0d8b30e07d81fe638dc578` | PASS |
| `07-timestamps` | `bb3d4b39aeda770bcd17d5b960f343d2c9de818f` | `0dbe1d13beae741f44bf7c027c7f083b71c6364b` | `c014e0011f8508b0f538091fcea53f3b9fcec58f` | `38ed64665b1d3373aa0df4418554cbf00c4ff55a` | PASS |
| `08-hw-debug-jtag` | `eab555d8b123b04f22d7ae5d8309a9d9f3eacc42` | `81c7b8dbec6434c25984f1ff6f3423e4141cbe98` | `322c933f02c5b5d669029dff46d1fb523851b77c` | `0b9f3cc4b3620ec6f8a7de31af826fcd9ae0b74a` | PASS |
| `09-privilege-level` | `a61b350a5e6a5b69d2a641a1920e923bafa37db3` | `ed3358701bb88fd340b227148db0be54d8a66fea` | `3c99184ee54acf905d7d9fea3e5ea75f6753fa52` | `ebb5e012c0e9ed61f04c29f1e632bff873abaeb1` | PASS |
| `10-virtual-mem-part1` | `6e43ab841b78645193bbd6bc01d6890f647a819c` | `c35a1b987f13251b430914515efd2499f74ddd50` | `d9f6b4d7c42a4bc3a6aeac3e894d129f4b51dd50` | `3c6d8ec7b1b536bc4b709e3340536da4643ce5e5` | PASS |
| `11-exceptions-part1` | `ec3d9488bf5ce3158e4645c76548b89e3a5c411f` | `b516961a5c17f6c6b027ec8cab59ca60a1de6bc6` | `58e6d1231ce38e3e6e3989648557aaa48a055670` | `a280dee0a310bd9da30fc761269e49cffe6e5448` | PASS |
| `12-integrated-testing` | `aa903a951d5819530697340e6621ad3293b6a491` | `8ad0e429a07881341aece810582b05298d02b5dc` | `006ac4da1993a04973419085baefe2747c4c0f64` | `6ab28f615a74563b300c3342d4d2405f667adb82` | PASS |
| `13-exceptions-part2` | `ecd4be6c959d318e2103aef253500d570c475d00` | `11664f6e2657b11f29a3a15438f670ad3baf5e27` | `af7a3894be56f04cd8702842fcbd5c7dc055171b` | `5665df07b9faefb0de921e7fbab5bd57629a50e0` | PASS |
| `14-virtual-mem-part2` | `c793b5c88619f57803f09e159574debe1b8f804b` | `f807e42df52163b7a8e976f84f12c98147f3d5f2` | `2a8172258a9bd2dfa7312b4f26b935468ab2d7c7` | `b21443330126da534faa27d753b6700dc17e9368` | PASS |
| `15-virtual-mem-part3` | `e91574c1946c1829ef4b50231d134728d9286569` | `957e29ec015161650f0cbb864d5bb7622d2f2386` | `32123539c2657f9f7dbd4205c12544a89daf3c11` | `8b4060464a2ae4940a1b27a9609f51831980d959` | PASS |
| `16-virtual-mem-part4` | `b635286af3bea918510f1e1ebf93715873b5bdc0` | `463a8e0e1517d5e757b714359a96a48c87f85346` | `13fe70187ab580d640fab82bd19f65912a059c28` | `c2425f39ff741204d5f8b950e677800f5f224d11` | PASS |
| `17-kernel-symbols` | `a6e97fe4ea75d78f345939de1e406eb1c676d238` | `9dcf3e1e0c659a3c6bbf209a47c38d8a8c2b2353` | `0b9cf59c344f44e31e93e37ac2c0c6359320b369` | `85f928478b13111578d7c32cbec6520e3c4cce1c` | PASS |
| `18-kernel-heap` | `542af98367574a551bc97a1763f3889121ab0fdb` | `f1176e0be91531acee86affb2166a1e7786ad863` | `65205852e30d4d2471210d47c02ea205f569bfff` | `9dffe6c82e4a870036672f00ffe2e8c105e3c853` | PASS |
| `19-timer-callbacks` | `a93ac20f34345017aa29740fc20f8d9e45f7bee3` | `a5582d517f7f18431c717d01f722f97cc591ec81` | `fd06d6ed7e71c52ed72d60d795796ef90c0f0a26` | `f0f37ae467eed35b8ab5cc0f34659be6a1be7ae5` | PASS |
| `20-boot-improvements` | `30b3d001dbcab7eae4df8710623bea016e3aed5e` | `19cfac2c793c752ec3c6445e139a302f1617d62d` | `d40cb16f20cba36bf86703d172350d0f11e3f093` | `3b6bd92d6e9dff5d405b6ae7bcfc2b36e273ee25` | PASS |
| `21-second-core` | `3633947e2471dec647168e67f8679ebdd130fcb5` | `10b3adf11e8845bd19807f0e2a7719176d71c60d` | `4871ece1bf56354b73ea51780394c7d29949b94e` | `4ef61696d48ae5c88bbf146a3a636f04f90596f9` | PASS |

## Validation

The branch chain contains exactly 21 post-boundary commits, and every chapter tip is the direct
parent of the next chapter's single commit. The original cleanup matrix recorded 228 passing checks
and no audit-only failures. It covered change-scope checks, strict stable formatting, Raspberry Pi 3
and 4 builds and Clippy targets, early QEMU smoke or boot tests, stable integration tests from
Chapter 12 onward, chainloader builds from Chapter 06 onward, and JTAG preparation from Chapter 08
onward.

A subsequent board-selection audit verified `BSP=rpiz2`, `BSP=rpi3`, and `BSP=rpi4` configuration
at every chapter tip. Clean Raspberry Pi Zero 2 W builds and Clippy checks passed for all 21
chapters; applicable QEMU tests, stable integration tests, persistent chainloader builds, and JTAG
configuration selection also passed. Chapters 17 through 21 additionally passed their test suites
from clean target directories without relying on symbol-tool artifacts from an earlier build.

From Chapter 12 onward, `make clippy` checks the bare-metal kernel with the kernel documentation
policy and separately checks an explicit list of native host tools under the host target. The host
list expands when Chapters 15 and 17 add tools.

The original history is reachable at `archive/2026-08-19/*`, and curated v1 is reachable at
`archive/2026-08-19-curated-v1/*`. Independent recovery bundles exist at
`/usr/local/src/fp/flamingos-preview-pre-cleanup-2026-08-19.bundle` and
`/usr/local/src/fp/flamingos-preview-curated-v1-2026-08-19.bundle`. The independent clone at
`/usr/local/src/fp/flamingos-preview.bak2` preserves the original pre-cleanup checkout.
