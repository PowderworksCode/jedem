# calquer

> Expose a Rust function once; call it from every language, with its shape intact.

A **calque** is a loan translation — a phrase borrowed by translating its parts
structurally rather than transliterating it. That is what this project does to a
function signature: the shape is preserved, each part is rendered natively.

**Status: design only. There is no code yet.**

calquer takes ordinary Rust and projects its **functions** into other languages.
The user annotates a normal crate; calquer emits a surface description and
generates per-language bindings from it. Direction is Rust → others, one way.

It is the successor to [fluessig][], narrowed from "describe a typed entity graph
once, project it everywhere" down to functions alone. DDL, ORM models, format
codecs and the Arrow data plane are out of scope permanently.

## Design docs

| Doc | What it covers |
|---|---|
| [calquer design](./docs/calquer-design.md) | Scope, the hard three (callbacks / handles / streams), what's dropped, reuse, and the first end-to-end milestone |
| [jawohl 2.0 design](./docs/jawohl-2.0-design.md) | calquer's first consumer — a cross-language incremental parser and validator for streaming structured data |
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
  the FFI boundary. calquer widens this to value-returning, fallible callbacks,
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
