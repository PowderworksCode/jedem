# jedem

> Expose a Rust function once; call it from every language, with its shape intact.

**jedem** is German for *to each* — the dative of *jeder*, "every". One Rust
function, handed to each language in its own idiom.

Home: **jedem.dev**

**Status: design only. There is no code yet.**

jedem takes ordinary Rust and projects its **functions** into other languages.
The user annotates a normal crate and runs one command; jedem reads the
descriptors the macros produced and writes the bindings. No intermediate file, no
interchange format. Direction is Rust → others, one way.

It is the successor to [fluessig][], narrowed from "describe a typed entity graph
once, project it everywhere" down to functions alone. Two in-house consumers
drive it: **pidgin** (~157 hand-written binding symbols that are almost entirely
generatable) and **jawohl 2.0** (a streaming parser that must feel native in
Python, TypeScript and .NET).

## Design docs

| Doc | What it covers |
|---|---|
| [jedem design](./docs/jedem-design.md) | Scope, the hard three (callbacks / handles / streams), what's dropped, reuse, and the first end-to-end milestone |
| [jawohl 2.0 design](./docs/jawohl-2.0-design.md) | jedem's first consumer — a cross-language incremental parser and validator for streaming structured data |
| [fluessig reading notes](./docs/fluessig-reading-notes.md) | The evidence base: what fluessig already solved, what carries over, and the restart-vs-narrow analysis |

Read them in that order. The design docs mark their confidence inline —
**[verified]** for claims run or read directly, **[speculation]** for unproven
reasoning.

## Scope: the simple things first

**v1 is a function that takes and returns plain values.** Free functions and
methods; records, enums, unions and scalars as parameters and returns; sync by
default with an async opt-out; fallible or infallible. Two targets: node/TS and
python.

If it needs a callback, a handle to a stateful object, or a stream, it is not v1.
All three are designed — see below — but none gates the first release. DDL, ORM
models, format codecs, the Arrow data plane and MCP generation are out
permanently.

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
