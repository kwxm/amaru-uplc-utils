# Building and running `amaru-uplc-utils`

## Requirements

- **Rust nightly.** `amaru-uplc` uses `#![feature(allocator_api)]` and edition
  2024, so a stable toolchain will not compile it. This project was tested with
  `rustc 1.102.0-nightly (739adecd4 2026-11-24)`.
- **rustup** (recommended). The checked-in `rust-toolchain.toml` selects a
  nightly toolchain automatically, so no `rustup override` is needed when you
  run Cargo from this directory.
- **A C toolchain** (`cc`). Some transitive dependencies of `amaru-kernel`
  build native code (for example the secp256k1 bindings). A normal Linux
  development environment is sufficient; no other system packages were needed
  in testing.
- **GHC and cabal** (optional). Only needed to generate a fresh set of test
  cases; see below.

To install nightly and make it the default:

```console
$ rustup toolchain install nightly
$ rustup default nightly        # only if you are not relying on rust-toolchain.toml
```

If Cargo reports "the option `Z` is only accepted on the nightly compiler" or
an error about `#![feature]`, you are on a stable or non-rustup toolchain.

## Building

```console
$ cargo build --release
```

This produces three binaries in `target/release/`:

| binary | purpose |
| --- | --- |
| `run-flat` | evaluate one flat-encoded UPLC program |
| `run-uplc` | evaluate one textual UPLC program |
| `run-conformance` | check a tree of generated cases against their expectations |

A debug build (`cargo build`) works too, but evaluation is noticeably slower.

## Generating test cases

The cases are produced by `generate-conformance-tests` in the Plutus repository.
From the root of that repository:

```console
$ cabal build generate-conformance-tests
$ cabal run generate-conformance-tests -- \
    --output-dir /tmp/cases --num-files 50 --format both --seed 1
```

`--format both` writes `.flat` and `.uplc` inputs for each case; `--num-files`
sets the number of cases per property; `--seed` makes the run reproducible.
The tool refuses to write into a non-empty directory unless `--force` is given,
and `--clean` removes cases from a previous run first. It also writes an
`info.json` describing the run.

Adding `--full-check` makes generation much slower (it typechecks every case),
but by default generated cases are still checked against the property they come
from.

## Running

```console
$ ./target/release/run-flat  /tmp/cases/integer/RingProperties/addition_associative/addition_associative-00001-A/addition_associative-00001-A.flat
$ ./target/release/run-uplc  /tmp/cases/integer/RingProperties/addition_associative/addition_associative-00001-A/addition_associative-00001-A.uplc
$ ./target/release/run-conformance /tmp/cases
```

`run-flat` and `run-uplc` print the program version, the consumed budget, the
evaluation outcome and, on success, the result encoded as flat. They accept an
optional Plutus version (`v1`, `v2` or `v3`; default `v3`).

`run-conformance` walks the case tree and, for every `<case>.flat` or
`<case>.uplc`, compares the evaluation result with `<case>.flat.expected` or
`<case>.uplc.expected`, and the budget with `<case>.budget.expected`. Useful
options:

| option | meaning |
| --- | --- |
| `--plutus-version v1\|v2\|v3` | semantics and default cost model (default `v3`) |
| `--no-budgets` | compare results only |
| `--filter TEXT` | run only cases whose path contains `TEXT` |
| `--verbose` | print a line for every passing case |

Exit status: `0` if everything passed, `1` if an evaluation or comparison
failed, `2` for usage, parse, decode or I/O errors.

## Notes

- Programs are decoded with `ProtocolVersion::default()` and evaluated with
  Amaru's default cost model for the selected Plutus version.
- With `amaru-uplc` 0.6.0 and Plutus V3, that cost model agrees with the
  reference implementation's testing parameters, so the generated
  `.budget.expected` files match. If a cost model changes, run with
  `--no-budgets` while investigating.
- `amaru-uplc` provides a textual parser but no textual printer, which is why
  both single-file runners print the result as flat hex.
