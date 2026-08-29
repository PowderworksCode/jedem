# jedem

> Expose a Rust function once; call it from every language, with its shape intact.

**jedem** is German for *to each* — the dative of *jeder*, "every". One Rust
function, handed to each language in its own idiom.

Home: **jedem.dev**

**Status: the spine works end to end.** One Rust function, generated Python
bindings, a real Python process calling them — see `demo/`. Four more languages
and the richer type vocabulary follow.

```sh
./demo/hello-py/run.sh    # generate, build, and run Python against it
```

jedem takes ordinary Rust and projects its **functions** into other languages.
The user annotates a normal crate and runs one command; jedem reads the
descriptors the macros produced and writes the bindings. No intermediate file, no
interchange format. Direction is Rust → others, one way.

It is the successor to [fluessig][], narrowed from "describe a typed entity graph
once, project it everywhere" down to functions alone. Two in-house consumers
drive it: **pidgin** (~157 hand-written binding symbols that are almost entirely
generatable) and **jawohl 2.0** (a streaming parser that must feel native in
Python, TypeScript and .NET).

## Development

`scripts/dev.sh` points git at the committed hooks, builds and tests the
workspace, installs the `cargo jedem` subcommand the demos are driven through,
and runs each round trip whose runtime is installed on this machine.

```sh
scripts/dev.sh
```

## The design

One doc: **[DESIGN.md](./DESIGN.md)** — what jedem is and why, the model, the
type system, ops, the three hard problems, backends, v1 and the roadmap.
Confidence is marked inline: **[verified]** for claims run or read directly,
**[speculation]** for unproven reasoning.

## Scope: the simple things first

**v1 is a function that takes and returns plain values** — `bool`, integers,
`f64`, `String`/`&str`, `Vec<u8>`, `Option<T>`, `Vec<T>`; synchronous; fallible
or not. Initial targets: **python, TypeScript, PHP, Java**, plus Rust, which
needs nothing generated.

If it needs a callback, a handle to a stateful object, or a stream, it is not
v1. All three are designed — see below — but none gates the first release. DDL,
ORM models, format codecs, the Arrow data plane and MCP generation are out
permanently.

There is no fallback for a type jedem cannot lower: it is a **compile error at
the macro**, never an opaque blob smuggled across as a string.

## The three hard problems

Exposing a function stops being simple at exactly three places, and fluessig's
notes are the evidence base for all three:

- **Callbacks** — the host supplies a closure. The Rust core sees one uniform
  shape regardless of source language; each backend wraps its native callable at
  the FFI boundary. jedem widens this to value-returning, fallible callbacks,
  restricted to synchronous ops.
- **Handles** — an op returns a live, method-bearing object. The core returns the
  core object; the binding wraps it into the generated class.
- **Streams** — an op yields values over time, as a native async iterable, with a
  per-op choice between throwing and errors-as-events.

## First milestone

One Rust function, one target language, actually callable: jawohl's
`complete_json` in Python — which also closes a promise jawohl's README has
carried unfulfilled since May 2023.

[fluessig]: https://github.com/zmaril/fluessig
