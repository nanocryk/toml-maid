# Toml Maid - Keep your TOML files clean

[![toml-maid crate](https://img.shields.io/crates/v/toml-maid.svg)](https://crates.io/crates/toml-maid)
[![toml-maid documentation](https://docs.rs/toml-maid/badge.svg)](https://docs.rs/toml-maid)

This formatter tries to apply an opinionated consistent formatting style.

Mainly, it considers lines not separated by blank lines as blocks, such that
sorting is only applied inside each blocks. It matches the practice in some
big Rust repositories to separate dependencies in sections, when many
other formatters don't take that into account and scramble the sections.

## Installation

```
cargo install toml-maid
```

## Usage

Run `toml-maid <my_file.toml>` to format a file. Many files can be provided.
Use the `--folder <path>` option to register a folder that `toml-maid` will scan
recursively for any TOML file (except `toml-maid.toml` files). Both can be used
together. If neither are used then the current folder is registered (equivalent
to `toml-maid --folder .`)

The `--check` option allows no modifying any file, and will instead exit with
an error code if a file is not well formatted. The `--silent` options allows
not outputing unimportant messages.

## Configuration

Behavior of `toml-maid` can be configured using a `toml-maid.toml` file, which
can be located in the current path or any parent folder, the first encountered
being used and others ignored. The options are the following:

- `keys`: list of keys as strings that should be sorted first in non-inline
  tables (`[section]` and `key = { ... }` entries). This can be used to keep
  important entries first.
- `inline_keys`: same but for inline tables `foo = { key1 = .., key2 = ..}`.
- `sort-arrays`: boolean telling if arrays should be sorted. Should only be used
  if order is not important, for exemple is suitable to keep `Cargo.toml`
  list of features ordered.
- `excludes`: list of patterns to ignore when scanning directories

## TODOs

- Improve comments formatting in multi-line arrays, mainly always move comments
  after the comma.
- Allow to configure ignored folder when scanning folders recursively (for
  exemple in this repo the `tests/output_consistency` folder should not be
  formatted as it contains by design non-formatted files).
- Add cute anime girl to README