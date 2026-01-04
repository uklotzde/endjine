<!-- SPDX-FileCopyrightText: The endjine authors -->
<!-- SPDX-License-Identifier: MPL-2.0 -->

# endjine

[![crates.io](https://img.shields.io/crates/v/endjine.svg)](https://crates.io/crates/endjine)
[![Docs](https://docs.rs/endjine/badge.svg)](https://docs.rs/endjine)
[![Dependencies](https://deps.rs/repo/github/uklotzde/endjine/status.svg)](https://deps.rs/repo/github/uklotzde/endjine)
[![Testing](https://github.com/uklotzde/endjine/actions/workflows/test.yaml/badge.svg)](https://github.com/uklotzde/endjine/actions/workflows/test.yaml)
[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-blue.svg)](https://opensource.org/licenses/MPL-2.0)

A Rust crate for managing the [Engine DJ](https://enginedj.com/) library database.

## Development

### Getting Started

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. Install Python as a prerequisite for [pre-commit](https://pre-commit.com/)
3. `cargo install just`
4. `git clone --origin upstream git@github.com:uklotzde/endjine.git`
5. `cd endjine`
6. `just setup`
7. `just`

If you plan to contribute then fork this repo on GitHub and add your fork as a remote named
`origin`.

## CLI

See the `endjine-cli/` directory for a simple CLI to perform basic tasks.

```shell
cargo run -p endjine-cli -- --help
```

All modifying tasks are considered as experimental. Always keep a backup of the original database
file!

## Naming

A phonetic wordplay and merge of the two words "Engine" and "DJ".

## License

Licensed under the Mozilla Public License 2.0 (MPL-2.0) (see [MPL-2.0.txt](LICENSES/MPL-2.0.txt) or
<https://www.mozilla.org/MPL/2.0/>).

Permissions of this copyleft license are conditioned on making available source code of licensed
files and modifications of those files under the same license (or in certain cases, one of the GNU
licenses). Copyright and license notices must be preserved. Contributors provide an express grant of
patent rights. However, a larger work using the licensed work may be distributed under different
terms and without source code for files added in the larger work.

### Contribution

Any contribution intentionally submitted for inclusion in the work by you shall be licensed under
the Mozilla Public License 2.0 (MPL-2.0).

It is required to add the following header with the corresponding
[SPDX short identifier](https://spdx.dev/ids/) to the top of each file:

```rust
// SPDX-License-Identifier: MPL-2.0
```
