# Agent field guide

Durable notes for anyone — human or agent — starting work in this repository.
Append what you learn; keep it to things that are true and not obvious from the
code.

## What this is

jedem takes ordinary annotated Rust and projects its *functions* into other
languages. The macros expand to pure data — a `&'static` descriptor inside the
user's own crate — and a generator reads that data and writes a whole binding
crate. There is no interchange file and no intermediate format, and the
direction is one way: Rust to everything else.

Today it generates two backends, Python (pyo3) and Node (napi-rs). The v1 type
vocabulary is plain values only; a type jedem cannot lower is a compile error at
the macro, never an opaque blob smuggled across as a string.

## Layout

A three-crate workspace plus three demo crates, all workspace members.

| crate | what it is |
| --- | --- |
| `crates/jedem-macros` | `#[jedem::export]`, `#[derive(jedem::Enum)]`, `jedem::surface!`. Expands to data only, never behaviour, and never writes a file at expansion time. |
| `crates/jedem` | the descriptor types and the generators. `src/gen/mod.rs` holds `Target` and the crate-emitting logic; `src/gen/python.rs` and `src/gen/node.rs` are the backends. |
| `crates/cargo-jedem` | the `cargo jedem` subcommand — `generate` and `run`. |
| `demo/hello` | the demo *surface*: ordinary Rust with one attribute, no FFI anywhere. Also where the tests live. |
| `demo/python`, `demo/node` | fully generated binding crates, committed. |

The backends are deliberately independent rather than sharing a
lowest-common-denominator spelling: the point is that each language gets what it
would have written, so the per-language differences are the product.

Design document: `notes/DESIGN.md`. Confidence is marked inline — `[verified]`
for what was run or read, `[speculation]` for reasoning. README and
`crates/jedem/src/lib.rs` link to it.

## Building and testing

Stable toolchain, no `rust-toolchain.toml`, no sibling checkouts. What CI runs
(`.github/workflows/ci.yml`):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

and, in a second job, the end-to-end round trips — which need Python 3.12 and
Node 22 on the machine:

```sh
cargo install --path crates/cargo-jedem
cd demo/python && cargo jedem run --python test.py
cd demo/node   && cargo jedem run --node   test.mjs
```

`cargo jedem run` builds the cdylib, finds it under `target/`, renames it to
whatever that runtime expects to import, and runs the script. It is not a
convenience wrapper over a script you could also run by hand — the eight lines
of shell it replaces were deleted on purpose, so read
`crates/cargo-jedem/src/host.rs` rather than reinventing them.

`fleet-lint.yml` is distributed by conf; a local edit here is drift the next
fleet sync reports. Its `hawk` job pins Rust 1.98.0, runs only when Rust changed,
and is advisory — it never fails the build.

**A plain `cargo build` at the workspace root can fail for a reason that has
nothing to do with your change.** `demo/python` is a workspace member and pins
`pyo3 = "0.23"`, whose build script refuses an interpreter newer than 3.13. On a
machine whose `python3` is 3.14 the whole workspace stops with

```
the configured Python interpreter version (3.14) is newer than PyO3's maximum
supported version (3.13)
```

CI does not hit this because its `roundtrips` job sets up Python 3.12. Locally,
either put a 3.13-or-older interpreter first on `PATH`, set
`PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1`, or build the crates you are working on
with `-p`. Note that `demo/python/Cargo.toml` is generated, so bumping the pyo3
requirement means editing the generator in `crates/jedem/src/gen/python.rs` and
regenerating, not editing the manifest.

## Landmines

**Generated code is committed, and a test diffs against it.**
`demo/hello/tests/generated_is_current.rs` regenerates every backend *in memory*
and compares byte-for-byte against what is on disk, for the whole generated
crate — manifest, shims and build script, not just the binding source. Change
the surface or the generator and the fix is:

```sh
cd demo/hello && cargo jedem generate
```

The generator bin is `demo/hello/src/bin/jedem-generate.rs`, whose `out: ".."`
is why it writes into `demo/python` and `demo/node`.

That same test file carries two guards worth not deleting. One asserts that the
set of backends with a drift guard equals `jedem::Target::ALL`, so adding a
third language fails until someone adds its guard. The other asserts every
generated file starts with an `@generated` marker in its first three lines.
`.gitattributes` marks `demo/python/**` and `demo/node/**` as
`linguist-generated`, so they collapse in review.

**Four multi-megabyte build artifacts are tracked in git.**
`demo/python/.jedem/hello.so`, `demo/python/.pyimport/hello.so`,
`demo/node/.jedem/hello.node` and `demo/node/.nodeimport/hello.node` are ~8-9 MB
ELF x86-64 objects committed to the tree, left over from the rename of
`.pyimport`/`.nodeimport` to `.jedem`. `.gitignore` lists `.jedem/`, which does
nothing for a path already tracked. The consequence to expect: running
`cargo jedem run` locally overwrites `.jedem/hello.so` and it shows up as a
modified binary in `git status` — on a non-Linux machine it will be a Mach-O or
a DLL. Do not commit it. Removing these from the tree is a separate change from
whatever you are doing.

**Two paths in prose are stale after that rename.** README's quick start says
`./demo/hello-py/run.sh` and `demo/hello/src/lib.rs` says the binding lives "next
door in `hello-py`". Neither path exists; the working commands are the
`cargo jedem run` invocations above.

**The generator has to build and run the user's crate.** A surface is `&'static`
data inside a compiled crate, so nothing can read it by inspecting source —
`cargo jedem generate` runs the crate's `jedem-generate` bin by convention.
There is no way to make generation a pure source transform, and the module doc
on `crates/cargo-jedem/src/main.rs` explains why.

## Where the interesting cases are

`demo/hello/src/lib.rs` is not a toy: it is the coverage surface, and each
function is there for a reason stated in its doc comment — a pinned export name
(`#[jedem(name = "shout")]`), a `Result` with a concrete error, a `Result` with
`Box<dyn Error>` (accepted because no backend inspects the error type; each
renders failure in its own mechanism carrying the `Display` text), `Option`,
`Vec<T>`, `&[u8]`, and an enum exported in both argument and return position.
Adding a type to the vocabulary generally means adding a case here first.
