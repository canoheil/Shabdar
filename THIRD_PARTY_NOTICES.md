# Third-party notices

## Phase 11 image-decoder Cargo dependencies

The Phase 11 lockfile adds 18 resolved entries from 17 unique packages
(`miniz_oxide` occurs at two versions). Exact-version package manifests were
checked; their declared license choices are permissive and none is copyleft:

| Package(s) | Declared license |
|---|---|
| `adler2` | `0BSD OR MIT OR Apache-2.0` |
| `bytemuck` | `Zlib OR Apache-2.0 OR MIT` |
| `byteorder-lite` | `Unlicense OR MIT` |
| `crc32fast`, `fdeflate`, `flate2`, `image`, `image-webp`, `png` | `MIT OR Apache-2.0` |
| `miniz_oxide` | `MIT OR Zlib OR Apache-2.0` |
| `moxcms`, `pxfm` | `BSD-3-Clause OR Apache-2.0` |
| `quick-error` | `MIT/Apache-2.0` (upstream expression) |
| `simd-adler32` | `MIT` |
| `zlib-rs` | `Zlib` |
| `zune-core`, `zune-jpeg` | `MIT OR Apache-2.0 OR Zlib` |

These crates remain Cargo-resolved dependencies; this change does not vendor
their source or copy new upstream assets into the repository. Release packaging
that distributes compiled binaries must retain the applicable license texts,
copyright notices, and any upstream `NOTICE` files for the complete resolved
dependency tree. This source-tree audit is not a substitute for generating the
release artifact's complete notice bundle.

## Persian bad-words seed data

The static moderation-rule seed in `apps/api/migrations/0010_comment_moderation.sql`
contains normalized and deduplicated literal strings from:

- Project: https://github.com/kaveh-dev/persian-bad-words
- Pinned commit: `4172f148dd9de014494b12693382cdad2ba1f2f5`
- Source file: `src/Data/farsi.json` (`farsiWords` and `finglishWords`)
- License file: `LICENSE`

No upstream JavaScript or Node package is included. The upstream MIT license is
reproduced in full below.

---

MIT License

Copyright (c) 2022 Thekawasaki

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
