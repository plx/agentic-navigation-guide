# Formal verification with Verus

This directory holds the [Verus](https://github.com/verus-lang/verus) harness
for the formally verified parts of `agentic-navigation-guide`. It is not part
of the Cargo package and adds nothing to the product's dependency graph.

## How it works

The verified code is ordinary production code. Functions in `src/proven/`
(and the few verified in place, listed below) carry their specification in a
`#[cfg_attr(verus_keep_ghost, verus_spec(...))]` attribute, and their spec
functions and lemmas live in `#[cfg(verus_keep_ghost)] verus! { ... }` blocks.
Verus compiles with `--cfg verus_keep_ghost`; ordinary `cargo build` never
sets that flag, so the product sees plain Rust with no Verus dependency.

[`verus_root.rs`](verus_root.rs) is the Verus crate root. It includes the
product source files at their real paths, under the same module names that
`src/main.rs` uses, so Verus checks exactly the code that Cargo compiles.

For every annotated function, Verus proves:

- functional correctness against its `ensures` specification;
- absence of panics, arithmetic overflow, and out-of-bounds indexing;
- termination of every loop and recursive function.

Unannotated functions in the same files are outside the proof, and Verus
refuses to let verified code call them.

## Running the proofs

```bash
proofs/run.sh /path/to/verus        # or: just verify-proofs /path/to/verus
```

A successful run ends with `verification results:: N verified, 0 errors`.
The [`Verus Proofs`](../.github/workflows/verus-proofs.yml) workflow builds
the pinned Verus and runs the same command.

### Pinned toolchain

| Component | Pin |
|---|---|
| Verus | commit `48ed2e8a36716eecc22057111681d8d29993b701` |
| Rust (for Verus only) | `1.100.0`, from Verus's `rust-toolchain.toml` |
| Z3 | `4.16.0`, the `z3` binary from the PyPI wheel `z3_solver-4.16.0.0-py3-none-manylinux_2_27_x86_64.whl` (SHA-256 `afae2551f795670f0522cfce82132d129c408a2694adff71eb01ba0f2ece44f9`) |

The product itself still builds with Rust 1.85 or newer; only the proof
check needs the Verus toolchain.

### Building Verus locally

```bash
git clone https://github.com/verus-lang/verus.git
cd verus/source
git checkout 48ed2e8a36716eecc22057111681d8d29993b701
# Place a Z3 4.16.0 binary at ./z3 (see the pin above), then:
rustup toolchain install
cargo build --release -p rust_verify -p verus -p cargo-verus
./target-verus/release/cargo-verus build --release --manifest-path vstd/Cargo.toml
export VERUS_Z3_PATH="$PWD/z3"
```

Invoke `target-verus/release/verus` by that path (or through a symlink):
Verus locates its verified standard library next to the binary.

## Trusted base

A proof is only as strong as what it assumes. These kernels trust:

- the Verus verifier, the Rust 1.100 front end it embeds, and Z3;
- `vstd`'s specifications of the standard-library operations the kernels use
  (`str` as a sequence of Unicode scalar values and its UTF-8 encoding,
  `str::chars`, `String::push`, slice and `Vec` indexing and length,
  `char::is_whitespace` as the Unicode `White_Space` property);
- that the stable `rustc` used for the product (1.85 or newer) gives the
  same meaning to the verified Rust as Verus's front end does;
- the unverified glue that calls the kernels: reading the guide file,
  `str::lines`, mapping kernel verdicts to `SyntaxError` values and messages,
  and all filesystem access.

No kernel uses `assume`, `admit`, `external_body`, or
`assume_specification`.

## What is verified

<!-- The kernel inventory is completed with the verification work. -->

## What is not verified, and why

- **Filesystem access and trust checks.** Opening guides without following
  links, directory enumeration, entry classification syscalls, and identity
  checks depend on operating-system state that `vstd` does not model. The
  contract also excludes hostile concurrent mutation, which has no
  sequential specification.
- **Host path semantics.** `std::path` component parsing, Windows prefixes,
  `OsStr` conversions, and Unicode case mapping have no Verus model; kernels
  consume their already-split outputs instead.
- **Command-line parsing, environment defaults, logging, and output
  streams.** These are `clap`, `std::env`, `env_logger`, and formatting
  plumbing.
- **Resource exhaustion.** Allocation failure and stack depth are not
  modeled.

These areas remain covered by the test suite described in
[CONTRIBUTING.md](../CONTRIBUTING.md).
