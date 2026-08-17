# jedem — design

> Expose a Rust function once; call it from every language, with its shape intact.

**Status:** design; no code yet. This is the design doc — the whole of it.

Confidence marks: **[verified]** = run or read directly during the design
sessions; **[speculation]** = unproven reasoning, with its failure condition
where one is known.

---

## 1. What jedem is, and why

**What:** a normal Rust crate that also ships a native library in every other
language. The author annotates the crate and runs one command; each language
gets bindings it would plausibly have written by hand — sync functions stay
sync, errors raise natively, unions arrive as real discriminated unions.

**Why now:** two in-house consumers need exactly this and are blocked without
it. **pidgin** has ~157 hand-written binding symbols across node, python and
php that are almost entirely generatable. **jawohl 2.0** is a streaming parser
whose whole premise is feeling native in Python, TypeScript and .NET — its 1.0
README has owed Python and JS wrappers since May 2023.

**Why not uniffi:** the pitch is the same — uniffi proved it in production. But
uniffi's first-class targets are Kotlin and Swift; node/TS, both consumers'
first requirement, is third-party there; and the shapes our consumers need most
— value-returning callbacks, factory-minted handles, async-iterable streams
with a chosen error model — are exactly where we hold ~13,000 lines of proven,
consumer-driven backend code from fluessig. Adopting uniffi would mean starting
those from zero inside someone else's architecture. [speculation — positioning,
not benchmarked; revisit if the reuse claim in §6 fails.]

**Direction: Rust → others. One way.** The Rust crate is the source of truth
and the implementation. jedem never imports a foreign surface into Rust, never
generates Rust from a `.d.ts`, and has no IDL.

**Scope: functions.** DDL, ORM models, format codecs, the Arrow data plane and
MCP generation are out of scope permanently. jedem does one job.

---

## 2. The model

**What the user writes** — their normal crate, plus annotations:

```rust
use jedem::{export, surface, Record};

pub struct Completer { /* the user's real, hand-written engine */ }

#[export]
impl Completer {
    #[jedem(ctor)]
    pub fn new(schema: &str) -> anyhow::Result<Self> { … }

    /// Feed a chunk. Synchronous — the default.
    pub fn push(&self, chunk: &str) -> anyhow::Result<()> { … }

    pub fn snapshot(&self) -> Snapshot { … }
}

#[derive(Record)]
pub struct Snapshot { pub path: String, pub complete: bool }

surface! { name: "jawohl", version: "2.0.0", api: [Completer] }
```

*(This example shows the full product — a ctor plus methods is a **handle**,
§5.2. The v1-shaped subset is §7's example: free functions only.)*

**What the tool does.** The derives expand to `&'static` descriptor data —
pure data, no behavior, no files written by macros. `surface!` names the
explicit roots; records, enums and unions are collected by reachability from
the exported ops. `cargo jedem generate` runs a bin target that links the
generator as a library and calls it on those descriptors directly:

```sh
cargo jedem generate --python    # -> the pyo3 binding
```

**jedem serializes nothing.** There is no interchange document, no
`surface.json`, no schema to version, no loader re-validating a file Rust just
wrote. The pipeline is derive → descriptor →
generator → bindings, all in one process. Drift between declaration and
implementation is structurally impossible because the exported `impl` *is* the
implementation. Two auxiliary needs the document used to serve are met
directly: regression protection comes from goldening the **generated
bindings** (a stronger gate — it catches generator changes too), and
`--dump-surface` prints the descriptors for debugging — explicitly not an
interface anyone may generate from.

**The front end is rustc.** Every type in an exported signature is a real Rust
type the compiler already resolved. A typo is a rustc error with
rust-analyzer completion; "unrecognised type" is not a reachable state. The
macros are built on `syn` + `darling`, with no reflection substrate.

---

## 3. The type system

Everything that can appear in an exported signature:

| Type | Rust side | Crossing |
|---|---|---|
| **Scalars** | `i32`, `i64`, `f64`, `bool`, `String`, `bytes` (`Vec<u8>`) | native; `bytes` is position-aware — a param is a view (`Uint8Array`), a return is owned (`Buffer`) |
| **Semantic scalars** | newtypes: `struct Oid(Vec<u8>)` | as their carrier, typed in docs |
| **Records** | `#[derive(Record)]` plain structs | native objects/dataclasses |
| **Enums** | `#[derive(Enum)]` | native enums; wire values pinnable |
| **Unions** | `#[derive(Union)]` on a Rust enum | **structured discriminated unions, always** — napi `Either{N}` over tagged structs, Python tagged classes. There is no envelope mode. |
| **Nullability** | `Option<T>` | `T \| null` / `Optional[T]`; param optionality is preserved |
| **Handles** | an exported interface, by reference | a generated class holding the core (§5.2) |
| **Callbacks** | closure params | the host's native callable (§5.1) |

Two structural rules, both load-bearing:

**The IR holds references, not names.** `Type::Handle(InterfaceId)` can only be
constructed pointing at an interface that exists. "Does this op mint a handle?"
is answered by the type system at construction, not by a name-lookup against a
derived set.

**There is no `Json` carrier.** No type in the vocabulary degrades to a JSON
string; a type jedem cannot lower is a **spanned compile error** at the derive,
never a stringly fallback. A value crossing the FFI is typed, or the binding
does not exist. The one opaque crossing is
**`Foreign`** — for genuinely external host types (`http.Server`, a
`ChildProcess`) — and it is *declared* by the author at the signature, never
reached for by the generator. Whether `Foreign` survives v1 is open (§9).

---

## 4. Ops: kinds and projections

An op is described by exactly two things:

**Kind** — what the op *is*:

```
Ctor | Method | Factory | Stream | Subscription | Manual
```

- `Ctor` — builds the handle's core; the generated class's constructor.
- `Method` — `&self` on a handle, or a free function on a stateless group.
- `Factory` — returns a handle the *core* built (§5.2).
- `Stream` — yields values over time (§5.3).
- `Subscription` — takes one callback, returns a registration handle whose
  drop/`unsubscribe()` deregisters.
- `Manual` — hand-written per binding; the escape hatch, counted by §8's metric.

**Projection** — how it *crosses*:

- **Synchronous by default; `#[jedem(async)]` is the per-op opt-out**. Async-ness is declared in exactly one place and
  means the same thing on every backend. There is no surface-level default.
- **Infallibility is inferred** from the return type: `T` emits a no-throw
  binding all the way through the shared core trait; `Result<T, E>` keeps the
  native throw/raise seam. (Ruby is the honest edge: its argument marshalling
  can itself raise, so a true no-raise `-> T` is emitted only for
  zero-marshalling ops.)
- **Name pins** — `#[jedem(name = "…")]` reproduces an exact export spelling
  per backend; unpinned ops take each backend's idiomatic casing.
- **Illegal combinations are unrepresentable or rejected once**, with spanned
  compile errors at the derive and one re-check in the validator — not
  discovered flag-pair by flag-pair.

**Fail loud, never narrow silently.** Where a backend cannot express something,
it emits *nothing* plus an explicit skip-note — never a plausible-looking
binding that breaks the consumer's build downstream, and never a degraded
stand-in.

---

## 5. The hard three

Exposing a function stops being simple at exactly three places. fluessig's
notes are the evidence base for all three; jedem inherits the solved parts and
changes two things.

### 5.1 Callbacks

**The contract:** the Rust core sees **one uniform shape regardless of source
language**:

```rust
Box<dyn Fn(A) -> Result<R, CallbackError> + Send + Sync + 'static>
```

Each backend's *generated* glue wraps its native callable at the FFI boundary —
a JS closure via `ThreadsafeFunction` (non-blocking), a Python callable under
the GIL, a Ruby `Proc` through a GVL trampoline, a PHP `Zval`, a wasm `Closure`
kept alive for the registration's lifetime. The core never learns where the
closure came from. (.NET: expected to be a pinned delegate + `GCHandle`
[speculation — no fluessig precedent].)

**Callbacks may return values and fail** — jawohl's native validators require
it — under one rule that makes it safe everywhere, including PHP's
single-threaded runtime:

> **A value-returning or fallible callback may appear only on a synchronous
> op.** It is invoked re-entrantly, on the host thread, inside that call. On an
> async or stream op it is a compile error.

Genuinely *async* callbacks (the core awaits a host promise) are deferred: no
consumer needs one, and the workaround — two forward-only halves with external
correlation — is documented in the notes.

**PHP's standing limitation:** forward-only callbacks there are
sync-same-request-thread only; off-thread invocation is undefined behavior. The
generated binding carries a loud marker. This is the one place the runtime
genuinely cannot meet the contract, and it is surfaced, not smoothed over.

### 5.2 Handles

A handle is a live, method-bearing object. Two ways one is born:

- **Ctor:** the generated class's constructor calls the core trait's ctor and
  wraps the result.
- **Factory:** the *core* builds the core object and hands it back; the binding
  only wraps. The core trait therefore returns
  `anyhow::Result<Arc<core::<Iface>Impl>>` — **the core object, never the
  generated class**, which pure Rust cannot name. Getting this backwards is the
  trap the notes document.

A factory-born interface gets a handle class with methods and **no public
constructor**. Async methods *on* a handle work from day one; an async or
stream *factory* (the mint itself wrapped in a promise) is deferred — a compile
error directs the author to a sync factory plus an async method.

jawohl's first useful surface is exactly this shape:
`Stream::from_json_schema()` mints the handle; `push`/`snapshot`/`status` are
its methods.

### 5.3 Streams

The core primitive is a **blocking poll**: `PollStream::poll(&self, timeout)`,
with an idempotent `close()`. Each backend projects it idiomatically:

- **node:** a genuine `for await` async iterable (napi `async_iterator`),
  driven through `spawn_blocking` so the event loop never blocks. Backpressure
  is by protocol — one pull in flight by construction. Early exit and `Drop`
  both run `close()`. A plain `next(): Promise<T | null>` poll cursor is
  retained as the feature-independent fallback, because napi's async-iterator
  support is **experimental** — the cursor is the hedge.
- **python:** a generator; same close semantics.

**The error model is per-op, two modes, chosen by the author:**

- **Default — throw:** a mid-stream core failure rejects the pull; the
  `for await` loop throws. Safe by default, no silent swallow.
- **`#[jedem(stream_error)]` — errors as events:** the failure arrives as a
  terminal, *typed* event variant and the stream completes; the pull never
  rejects. For surfaces where failure is domain data — jawohl's
  `ValidationFailed` is the canonical case.

Construction-time errors always throw, in both modes.

---

## 6. Backends

Tiered by actual consumer demand, not ambition:

| Tier | Targets | Why |
|---|---|---|
| 1 | **python, TypeScript** | Both consumers need both. Everything ships here first. |
| 2 | **PHP, Java** | pidgin needs PHP; Java rounds out the initial set. |
| — | **Rust** | Nominally a target, but there is no FFI boundary between Rust and Rust: a consumer depends on the crate directly. "Rust support" means the annotations are **inert**, which `demo/hello/tests/rust_still_works.rs` asserts. Nothing is generated. |
| later | .NET, ruby, wasm, cpp | .NET is jawohl's fourth language and has no fluessig precedent; the rest are carried, not driven. |

**The inherited asset** [verified]: fluessig's `src/api.rs` + `src/bindgen/**`
— ~13,200 lines across seven backends, already independent of everything jedem
drops (the schema side reaches in through exactly two case-conversion helpers).
State of the hard three today:

| Backend | Callbacks | Handles | Streams |
|---|---|---|---|
| node | ✅ | ✅ | ✅ async-iterable |
| python | ✅ | ✅ | ✅ generator |
| ruby | ✅ | ⛔ skip-note | ✅ |
| cpp | ✅ | ⛔ | — |
| java | ✅ | ⛔ | poll cursor |
| wasm | ✅ | ⛔ | — |
| php | ✅ *sync-only* | ⛔ | — |

What does **not** carry over from those lines: every degrade path. The envelope
union projection, the unrecognised-scalar → `String` fallback, and the bare
`Json` cross-package carrier are deleted in the port;
java's union crossing (envelope strings today) becomes a skip-note until
structured lowering is built there. The typed lowering carries; the fallbacks
do not.

**The proof discipline carries too:** every backend keeps a runnable
round-trip — a real host process calling real generated bindings, in CI. A
backend without one does not count as done.

---

## 7. v1 and the road

**v1 is a function that takes and returns plain values.** Free functions;
`bool`, integers, `f64`, `String`/`&str`, `Vec<u8>`, `Option<T>` and `Vec<T>`
as parameters and returns; synchronous; fallible or infallible. Initial
targets: python, TypeScript, PHP and Java — plus Rust, which needs nothing
generated. Records, enums, unions and semantic scalars follow; async follows
them. If it needs a callback, a handle mint, or a
stream, it is not v1 — all three are designed (§5), none gates the first
release.

**The first thing that works** is jawohl's `complete_json` in Python — the
smallest honest end-to-end case (58 lines, zero dependencies, builds and passes
clean today [verified]), and it closes the README promise open since May 2023:

```rust
// jawohl/src/lib.rs — the existing function, plus four lines
use jedem::{export, surface};

pub struct Jawohl;

#[export]
impl Jawohl {
    /// Complete a partial JSON document by appending its missing closers.
    pub fn complete_json(input: &str) -> Result<String, MalformedJsonError> { … }
}

surface! { name: "jawohl", version: "2.0.0", api: [Jawohl] }
```

```sh
cargo jedem generate --python
maturin develop
```

```python
>>> import jawohl
>>> jawohl.complete_json('{"key": "value", "arr": [1, 2, {"nested": "v')
'{"key": "value", "arr": [1, 2, {"nested": "v"}]}'
>>> jawohl.complete_json('{"a": 1}}')
jawohl.MalformedJsonError: The input JSON string is malformed.
```

One free function, a `&str` param, a fallible return lowering to a real Python
exception, sync by default. Nothing async, no callback, no stream — the point
of a first milestone.

**Then, in order, each step adding exactly one hard thing:**

| Step | Adds | Proves |
|---|---|---|
| 1 | `complete_json` → Python | the spine: derive → descriptor → generator → backend → a real host process |
| 2 | the same function → node/TS | one surface, two languages, no second declaration |
| 3 | `Stream::from_json_schema()` → a handle with `push`/`snapshot` | **handles** (§5.2), incl. the factory mint |
| 4 | `stream.changes()` → `for await` / generator | **streams + events** (§5.3), errors-as-events |
| 5 | a native validator param | **value-returning fallible callbacks** (§5.1) |
| 6 | .NET | the new backend |

Steps 3–5 are the hard three in ascending order of risk, each against a real
consumer rather than a demo. Before the IR freezes, **author jawohl 2.0's and
pidgin's complete surfaces against it** — surface-first authoring is what
caught every gap in fluessig, and it is the cheapest insurance available.

**The standing tension:** jawohl is a full jedem consumer from day one — it
writes no bindings by hand — and nothing useful in jawohl fits inside the v1
boundary — only `complete_json` is v1-shaped. jawohl's useful surface waits for
steps 3–5; its Rust core (the majority of its engineering) is not blocked and
absorbs the wait.

---

## 8. The name is the metric

**jedem** — German *to each*, the dative of *jeder*. The name sets a countable
test: **does every language actually get it?**

Where the design meets it: the callback contract (one core shape, each
language's own native callable — nobody left out) and idiomatic spelling
everywhere (position-aware `bytes`, sync stays sync, structured unions, real
`for await` — each language gets the version *it* would have written).

Where it falls short, tracked: **skip-notes** (five backends emit nothing for a
handle mint today — an absence, the sharpest failure, and steps 3 and 6 exist
to shrink it); **`@manual`** ops (a hand-written binding is one place the
promise was not kept); **PHP's weaker callback semantics** (the one
runtime-imposed imperfection — surfaced loudly, never removable). And at v1 the
name is a promise the roadmap owes, not a description: two languages, done
properly, first.

**The metric:** per-backend **coverage** — what fraction of the declared
surface actually lowers, and how many skip-notes remain. Computed from the
in-memory descriptors (`--dump-surface`) against each backend's output. It
should only ever go up.

---

## 9. Open questions

1. **Does jawohl wait, or get a temporary hand-written binding?** Only
   `complete_json` is v1-shaped; the rest of jawohl's surface waits for steps
   3–5. Currently assumed: it waits. The release valve is a throwaway pyo3
   binding for two free functions.
2. **What replaces the entl/disponent parity gates?** fluessig's standing proof
   that a change did not silently alter output leaves with the entity graph.
   pidgin and jawohl should inherit the role, but neither has a committed
   golden yet — and a generator without a parity gate regresses quietly.
3. **Does `Foreign` survive v1?** Neither consumer's v1 surface needs an opaque
   external-type crossing, and the v1 boundary argues for cutting it until
   something does.
