# jedem — design

> Expose a Rust function once; call it from every language, with its shape intact.

**Status:** design. Written after reading fluessig's notes (see
[`fluessig-reading-notes.md`](./fluessig-reading-notes.md)) and the jawohl 2.0 doc.
**Basis:** genuine restart — fluessig's notes are the spec, fluessig's code is
prior art, not a starting point. *(Owner's call. My reading found no structural
defect that forced it; §6 records what the restart must therefore buy to be
worth its cost.)*

Confidence is marked throughout: **[verified]** = read or run in this session;
**[from notes]** = fluessig proved it, evidence in `notes/`; **[speculation]** =
my reasoning, unproven.

---

## 1. Scope

jedem takes **ordinary Rust** and projects its **functions** into other
languages. That is the whole product.

**Direction: Rust → others. One way.** The Rust crate is the source of truth and
the implementation. jedem never imports a foreign surface into Rust, never
generates Rust from a `.d.ts`, and has no IDL. *(fluessig had a converter —
`hinzu` — reading pi's TypeScript into an api surface. That is a separate tool
and out of scope here.)*

**What the user writes.** Their normal crate, plus annotations:

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

surface! { name: "jawohl", version: "2.0.0", api: [Completer], types: [Snapshot] }
```

Then `cargo jedem generate` writes the per-language bindings. One command, no
intermediate file. **No second copy of the model, and the exported `impl` is the
impl that actually runs** — declaration/implementation drift is structurally
impossible. [from notes: `derive-front-end.md` §2.7]

**Languages, tiered by actual consumer demand** — not by ambition. The two known
consumers are **pidgin** (~157 symbols, node + python + php, ruby soon) and
**jawohl 2.0** (Rust, Python, TypeScript, .NET, then others):

| Tier | Targets | Why |
|---|---|---|
| 1 | **node/TS, python** | Both consumers need both. Everything ships here first. |
| 2 | **.NET** | jawohl target #4. **Does not exist in fluessig** [verified] — a genuinely new backend. Technology is not prescribed: whichever Rust→C# bindgen actually works (`csbindgen`, `interoptopus`, or a hand-rolled C ABI + P/Invoke), chosen on contact rather than up front. |
| 3 | php, ruby | pidgin needs them; fluessig has both. |
| 4 | wasm, cpp, java | fluessig has them; no consumer is asking. Carried, not driven. |

**Explicitly not in scope:** anything about *data*. See §4.

### The v1 boundary — "the simple things"

Starting over is only worth it if the first version is genuinely small, so the
line is drawn explicitly rather than left to drift:

> **v1 is a function that takes and returns plain values.** Free functions and
> methods; records, enums, unions and semantic scalars as parameters and returns;
> synchronous by default with an async opt-out; fallible or infallible. Two
> targets: node/TS and python.

If it needs a **callback**, a **handle mint**, or a **stream**, it is not v1. All
three are designed in §2 — the notes already paid for that knowledge and throwing
it away would be the expensive mistake — but none of them gates the first release.
§7 sequences them one at a time, each against a real consumer.

MCP generation, the effects flags, and every data-shaped concern are gone
entirely (§4). What remains is one job done well.

**The tension this creates, stated up front:** jawohl 2.0 is a full jedem
consumer from day one, and *nothing useful in jawohl fits inside the v1 boundary*
— `Stream` is a handle mint, `changes()` is a stream, native validators are
callbacks. Only `complete_json` is v1-shaped. So either jawohl waits for §7 steps
3–5, or the day-one-consumer decision gets revisited. §7 assumes it waits; the
jawohl design's §8 costs that out.

---

## 2. The hard three

fluessig solved these; the notes are the evidence base and I am not re-deriving
them. What follows is what jedem **changes**, and why — and the driver for two
of the three changes is jawohl 2.0, which needs more than pi ever did.

### 2.1 Callbacks — solved, then widened

**Inherited unchanged** [from notes: `callback-function-types.md`]: the Rust core
sees **one uniform shape regardless of the source language**. Each backend's
*generated* glue wraps its native callable at the FFI boundary. The core never
learns whether the closure came from JS, Python, or Ruby. That contract is the
single most valuable thing in the notes and jedem adopts it verbatim, along
with the per-backend non-blocking table (node `ThreadsafeFunction` NonBlocking,
python `with_gil`, ruby GVL trampoline, wasm keep-`Closure`-alive, php `Zval`;
the .NET entry — a pinned delegate + `GCHandle` — is my extrapolation, since
fluessig has no .NET precedent [speculation]).

**Changed — and this is forced by jawohl.** fluessig's callbacks are
**forward-only, synchronous, void-returning**, and `load_api` *hard-rejects*
anything else [verified, `src/api.rs:585`]:

> `only forward-only sync void callbacks are supported (is_async/fallible/non-void returns not yet implemented)`

That was correct for pi: a source-level enumeration found pi has no
value-returning callback anywhere. **jawohl 2.0 is built on one.** A native
validator —

```python
@field_validator("username")
def username_available(value): return check_database(value)
```

— is a callback into the host that **returns a verdict and can fail**. jedem
cannot defer this; it is the feature.

So the uniform core shape widens:

```rust
// fluessig
Box<dyn Fn(A) + Send + Sync + 'static>
// jedem
Box<dyn Fn(A) -> Result<R, CallbackError> + Send + Sync + 'static>
```

**The rule that makes this tractable.** Calling *into* a host language and
waiting for a value is only safe when the call originates on the host's own
thread; from a Rust background thread it needs an async-oneshot bridge and, on a
single-threaded runtime, deadlocks. fluessig deferred the feature precisely
because of that. jedem takes it with a restriction instead:

> **A value-returning or fallible callback may appear only on a synchronous op.**
> It is invoked re-entrantly, on the host thread, inside that call. On an async
> or stream op it is a compile error.

This is checkable in the macro (spanned) and re-checked at load, exactly the
shape of fluessig's `single_threaded` rule [from notes]. **It is also precisely
jawohl's shape:** `stream.push(chunk)` is synchronous and host-called, native
validators run "once the relevant value becomes complete" — i.e. during that
push. Under this rule the feature is implementable on *every* backend including
PHP, whose single-thread request model made fluessig's off-thread callbacks UB.
[speculation — the rule is my design; the deadlock analysis behind it is from the
notes, but no code proves the widened shape yet.]

**Still deferred:** genuinely *async* callbacks (host returns a promise the core
awaits). No consumer needs one. Meanwhile: a host that wants async work in a
validator must do it itself and block, or use the two-forward-halves pattern the
notes document.

`Shape::Subscription` (register → handle whose drop/`unsubscribe()` deregisters)
carries over intact. jawohl's `ValidationFailed` listeners are exactly it.

### 2.2 Handles — solved, with the IR corrected

**Inherited unchanged** [from notes: `class-handle-return.md`] — including the
non-obvious crux that a restart could easily get wrong: **the core trait must
return the core object, `Arc<core::<Iface>Impl>`, not the generated handle
class.** A pure-Rust core cannot name or build a napi/pyo3 class; the *binding*
wraps. Also inherited: emit the handle class for anything constructible, not just
things with a ctor, and a factory-born class gets no public constructor.

**Changed — the one place the restart pays for itself.** fluessig spells a handle
return as `ApiType::Model { model }` — the *same* spelling as a plain DTO —
and tells them apart at lowering time by checking membership in the interface-name
set. The note is explicit that this was chosen so that "existing goldens with
DTO-returning ops are byte-identical." **jedem has no goldens.** So:

```rust
ApiType::Handle { handle: String }   // a live object with methods
ApiType::Record { record: String }   // a plain data struct
```

Two different things get two different spellings. The consequences are real, not
cosmetic: "does this op mint a handle?" becomes a **parse-time type question**
instead of a name-lookup against a derived set; `constructible_interfaces` and
`returned_interface_name` stop existing as concepts; and a `Handle` naming
something undeclared is an error rather than silently degrading into a DTO
reference. This is the clearest example of a decision fluessig could not take
and jedem can.

jawohl needs this on day one: `Stream::from_json_schema(schema)` is a factory
minting a stateful handle, and `stream.push` / `.snapshot()` / `.status(path)`
are its methods.

**Deferred:** an *async* or *stream* factory op (the mint itself wrapped in a
promise). Synchronous factories cover both consumers. Meanwhile: a would-be async
factory is a compile error directing the author to a sync factory plus an async
method.

### 2.3 Streams and events — solved; the error model is the valuable part

**Inherited** [from notes: `async-iterable-streams.md`]: node gets a genuine
`for await` via napi 3 `#[napi(async_iterator)]` over a **blocking**
`PollStream::poll`, driven through `spawn_blocking` so the event loop stays free;
backpressure is by protocol (one pull in flight by construction); cancellation
runs `close()` with `Drop` as a backstop, and `close()` must be idempotent.
Python gets a generator, and the poll cursor stays as a feature-independent
fallback.

**The risk, carried over honestly:** `#[napi(async_iterator)]` is
**experimental** in napi 3 and gated on `tokio_rt`. The retained poll cursor is
the hedge, and jedem keeps it for that reason. [from notes — flagged by the
note's own author.]

**Inherited, and it turns out jawohl wants it:** the **dual error model** —
streams use errors-as-events (a terminal error event, then completion; the pull
never rejects), unary and ctor ops throw, and construction-time errors always
throw. fluessig made errors-as-events an opt-in (`@streamError`) because
idiomatic TS prefers a throwing stream. **jawohl inverts that default for its own
surface**, and correctly: `ValidationFailed` is a *domain event*, not an
exception — the whole point of incremental validation is that the consumer keeps
receiving events and decides whether to cancel. So the per-op switch fluessig
built is exactly the right knob, and jawohl sets it the other way.

jawohl's event enum (`ValueStarted | ValueProgressed | ValueCompleted |
ValidationFailed | …`) lands as a structured discriminated union — napi
`Either{N}` over per-variant tagged structs. In fluessig that is the default
*with a JSON-envelope opt-out* (`{"kind": tag, "payload": body}` as a string)
[verified: `tests/union_structured.rs`]. **jedem deletes the envelope.** It is a
`Json` carrier by another name (§3), and a projection mode is the most expensive
kind of surface to keep — every backend must implement both forever. Structured
is the only union projection; a backend that cannot lower it yet emits a
skip-note, per fail-loud. Consequence, stated honestly: fluessig's java backend
crosses unions *as* envelope strings today, so java unions start as a skip-note
in jedem until structured lowering is built there — absent rather than degraded,
which is the §8 rule.

---

## 3. What the restart re-decides

Beyond §2.2's `Handle`, three IR changes that a restart can make and an
in-place narrowing could not:

**No serialized document at all.** fluessig splits `catalog.json` (entities,
enums) from `api.json` (ops, models), and an earlier draft of this doc proposed
merging them into one `surface.json`. Both are wrong. The JSON existed for
exactly one reason, stated in fluessig's own locked-decisions table: *"Language ↔
core interchange — `catalog.json`, versioned, fully resolved — Rust core never
embeds Node"* [verified, `notes/design.md:113`]. The front end was TypeSpec, a
Node program; the engine was Rust; JSON was how they spoke.

**TypeSpec was deleted. The boundary it crossed no longer exists.** With a Rust
derive front end and a Rust generator, `surface.json` is Rust serializing to JSON
so that Rust can immediately parse it back, inside one toolchain. It is a vestige
of a language boundary that was removed a hundred commits ago.

So jedem has no interchange document. The derive produces `&'static` descriptors;
a bin target in the user's crate links the generator as a **library** and calls it
on those descriptors directly. `cargo jedem generate` runs that bin. What
disappears with the JSON: a serde round-trip, a schema to version, the
`skip_serializing_if` house style that shaped every flag added to `ApiOp`, and a
whole class of "the checked-in document is stale" failure.

Two things the JSON was quietly doing, and their replacements:

- **The drift guard** diffed a regenerated catalog against a committed one.
  Replaced by goldening the **generated bindings** instead — a strictly better
  gate, because it catches generator changes too, not just front-end changes.
- **Debuggability** — "what did the macro actually see?" Kept as
  `cargo jedem generate --dump-surface`, explicitly a debug artifact and
  explicitly **not** an interface anyone may generate from.

Cross-crate surfaces get better rather than worse: crate B's descriptors are
`&'static` items that crate A links and references directly, type-checked by
rustc, instead of two JSON documents merged by name.

**Shape and projection are separated.** `ApiOp` accreted eight-plus flags one PR
at a time — `is_async`, `infallible`, `readonly`, `destructive`, `worker`,
`stream_error`, `result`, `bindings`, plus interface-level `single_threaded` —
each an "additive optional field with `skip_serializing_if`" to preserve goldens
[verified, `src/api.rs:154-229`]. They are three different kinds of thing mixed
in one struct, which is why `load_api` needs a pile of cross-field legality
checks. jedem separates:

- **kind** — what the op *is*: `Ctor | Method | Factory | Stream | Subscription | Manual`
- **projection** — how it *crosses*: async opt-out, result-envelope, name pins

There is **no third category.** fluessig has one — `readonly` / `destructive` /
`worker` — but those flags are consumed by exactly one thing: `src/bindgen/mcp.rs`.
Zero references in any other backend [verified]. With MCP dropped (§4) the whole
semantic-metadata axis goes with it, and an op is described entirely by what it
is and how it crosses.

Illegal combinations become unrepresentable or checked in one place, rather than
discovered flag-pair by flag-pair.

**No `Json` carrier, by construction.** fluessig degrades in two places when it
cannot type something: an unrecognised scalar maps to `String` at the shared type
chokepoint (`ty()`, `src/bindgen/mod.rs:433` — the note's own words: "the typed
methods on that object vanish"), and a cross-package type resolved without
`--context` degrades to a bare `Json` scalar [from notes:
`class-handle-return.md`]. Both exist because fluessig's front end could name
types it could not see. jedem's front end is rustc: every type in an exported
signature is a real Rust type the compiler already resolved, so "unrecognised"
is not a state that can occur. The rule, then, has three teeth:

1. There is **no `Json` type in the vocabulary** — nothing to degrade *to*. A
   type jedem cannot lower is a **spanned compile error** at the derive, never a
   stringly carrier.
2. The **union envelope projection is deleted** (§2.3) — structured lowering or
   a skip-note, nothing in between.
3. The only opaque crossing is **`Foreign`**, which is *declared* by the author
   for genuinely external host types — an explicit decision at the declaration
   site, never a fallback the generator reaches for. (Whether `Foreign` survives
   v1 at all is §9's open question 7.)

A value crossing the FFI is typed, or the binding does not exist. There is no
third state.

**Inherited deliberately, not re-decided** — these are notes decisions I
considered changing and kept, because the notes' reasoning survives:

- **Synchronous is the global default; async is a per-op opt-out.** Tempting to
  infer async-ness from the Rust `fn`, but that is wrong: an IO-bound *sync* Rust
  fn should still get a `Promise` so it doesn't block the event loop. Async must
  be declared, in exactly one place, meaning the same thing everywhere. No
  document-level lever. [from notes — an inversion fluessig was forced into by
  pidgin.]
- **Infallibility is inferred** from `T` vs `Result<T>`, and propagates into the
  shared core trait. Ruby stays the honest edge: its arg marshalling is itself
  fallible, so a true no-raise `-> T` is emitted only for zero-marshalling ops.
- **Derive → `&'static` descriptor → separate exporter.** Macros never write
  files. Explicit `surface!` root list, **not** `inventory`/`linkme`
  link-sections (flaky on wasm). `syn` + `darling`; no reflection substrate
  (`facet` is pre-1.0 with attributes "in flux"; `bevy_reflect` is a runtime
  system).
- **Fail loud, never silently narrow.** Where a backend cannot express something,
  emit *nothing* plus an explicit skip-note — never a plausible-looking binding
  that breaks the consumer's build downstream.

---

## 4. What is dropped from fluessig

All four are **never**, per the owner's decision. Stating them precisely, because
"never" is a scope commitment:

| Dropped | Verdict | What happens |
|---|---|---|
| **DDL** (`CREATE TABLE`, 3 dialects) | **never** | Deleted. `src/sql.rs`. |
| **ORM models** (SQLAlchemy, Django, TS tables, Drizzle) | **never** | Deleted. `src/codegen.rs`. |
| **Format codecs** (Mongo, JSONL, Parquet, Mermaid) | **never** | Deleted — and mostly never built past design. |
| **MCP surface generation** | **never** | Deleted. `src/bindgen/mcp.rs` (518 ln) turned an op surface into an MCP tool manifest. Real, but it is a *second product* wearing jedem's clothes, and starting over means starting simple. It takes the `readonly`/`destructive`/`worker` flags with it (§3) — nothing else consumed them [verified]. |
| **Arrow data plane** | **never** *as a data plane* | `src/data.rs` deleted. **But** `ArrowBatch` survives as an ordinary opaque type that crosses the FFI as bytes / `byte[]`, because it is just a type in someone's signature. That is not a data plane; it is jedem doing its one job on a type that happens to be Arrow. |

Dropped with them: the **entity graph** itself (`src/ir.rs`, `src/catalog.rs`),
`catalog.json`, the `Entity` / `Edge` / `AbstractRoot` derives, `Id<T>`,
`ref_cols`, `shares`, `flatten`, generated polymorphic key enums, and the
weak-entity / composition / column-parity machinery.

**Consequence, stated plainly:** **entl and disponent stop being consumers.**
They own their schemas. `crates/entl-schema-derive` (1,212 ln) and
`crates/disponent-schema-derive` (1,098 ln) exist only as entity parity gates and
do not carry over. This removes jedem's two oldest dogfood targets and replaces
them with pidgin and jawohl — which is the right trade, since both new ones are
*binding* consumers and the old ones were schema consumers.

`findings.md` dies with them, except three things worth carrying: the op-surface
section (the four shapes held against a real surface; **param optionality was
dropped by the extractor and a real emitter must keep it**), `@manual` earning
its keep, and above all **the method** — author the complete real surface before
freezing the IR. §7 applies it.

---

## 5. Reuse

39,225 lines of Rust in fluessig [verified]. Where it goes:

### Carries over largely intact — the two big assets

**The bindgen backends: 12,938 lines** [verified], and they are already
independent of everything being dropped. `src/api.rs` (785 ln) imports **only**
`std::collections::BTreeMap` and `serde`; `src/bindgen/**` imports from the
schema side **exactly two symbols** — `ir::camel` and `ir::snake`, across five
call sites [verified]. There is no other coupling to sever.

| Backend | Lines | Callbacks | Handles | Streams |
|---|---|---|---|---|
| node | 1,493 | ✅ | ✅ | ✅ async-iterable |
| python | 1,297 | ✅ | ✅ | ✅ generator |
| ruby | 1,366 | ✅ | ⛔ skip-note | ✅ |
| cpp | 1,482 (+433+679 hdr) | ✅ | ⛔ | — |
| java | 1,416 | ✅ | ⛔ | poll cursor |
| wasm | 835 | ✅ | ⛔ | — |
| php | 568 | ✅ *sync-only* | ⛔ | — |
| rust_core | 546 | — | — | — |

Two things this table makes obvious: **handle-minting exists on node and python
only** — five backends emit honest skip-notes — and **.NET is absent entirely**
[verified]. Those are jedem's two real engineering fronts, and jawohl needs
both (`Stream` handle × Python/TS/.NET).

"Largely intact" has one systematic exception: the §3 `Json`-carrier ban deletes
code from every backend in the port — the envelope union projection, the
unrecognised-scalar → `String` fallback, and the bare-`Json` cross-package
degrade all go, and java's union crossing (envelope strings) becomes a skip-note
until structured lowering is built there. What carries over is the typed
lowering; the degrade paths do not.

**The op-layer IR: 785 lines**, as *prior art*. Under a restart it is re-derived
(§3), but the type vocabulary — `Scalar`, `Model`, `Enum`, `List`, `Nullable`,
`Union`, `Foreign`, `Callback` — is proven against two real surfaces and the new
version is closer to a refactor of a known-good design than a blank page. Note
what shrinks in the port: much of `api.rs` is serde scaffolding — `untagged`,
`deny_unknown_fields`, `skip_serializing_if` on every added flag, `default` fns,
and a `load_api` that re-validates a document Rust just wrote. With no
interchange document (§3) that scaffolding has nothing to do.

**The op half of the derive front end.** Of `fluessig-derive` +
`fluessig-derive-macros` (4,916 ln, 7 derives), jedem keeps `#[export]`
(710 ln), `Record`, `Union`, `Enum`, `Scalar`, `catalog!`, plus spans, doc
capture and the drift guard.

**The runnable-proof discipline.** `crates/callback-demo-{node,py,php,ruby,wasm}`,
`cpp-demo`, `java-demo` — real host processes calling real generated bindings,
wired into CI. These are small (84–245 ln each) and they are the reason the
callback contract is credible rather than asserted. Carry the pattern; a backend
without a runnable round-trip does not count as done.

### Rewritten

The op IR (§3) — now plain in-memory Rust types rather than a serde schema,
since nothing serializes them (§3). The `surface!` exporter, the
`cargo jedem generate` driver (fluessig's two-step emit-then-generate CLI
collapses to one), and the loader — which shrinks a lot once the flag
cross-checks are structural and there is no untrusted document to validate.

### Abandoned

`src/{ir,catalog,sql,codegen,data}.rs` (~2,456 ln), the `Entity`/`Edge`/
`AbstractRoot` derive half, both schema-derive crates (2,310 ln), the frozen
entl/disponent catalog fixtures, and the entity-graph portions of `tests/`
(`entl_catalog`, `union_catalog`, `cpp_catalog`, `java_catalog`, `php_catalog`
≈ 1,600 ln).

**Rough accounting** [verified line counts, my allocation]: ~13,200 lines carry
over as the core asset, ~7,300 are abandoned outright (including MCP's 518), and
the rest is tests and demos that follow whichever half they served.

---

## 6. What the restart has to buy

Recorded because it is the decision everything else hangs off, and it should be
checkable later rather than assumed.

My reading found **no structural defect** in fluessig that forced a restart: the
function-exposure half is already cleanly separated [verified], it is already the
only part under development (last 60 commits: `src/bindgen/*` 87 touches,
schema files **zero**; last schema commit 2026‑07‑20, a lint chore, 80 commits
back) [verified], and fluessig has already survived a total front-end replacement
in place (TypeSpec built, dogfooded on two consumers, deleted) [verified].

The owner chose restart anyway. So the restart is worth its cost **iff** it
delivers the things a narrowing could not:

1. `Handle` as a first-class type instead of an overloaded `Model` (§2.2) — the
   one change fluessig's own note says it declined *only* for golden-compat.
2. Shape and projection separated — and the effects axis deleted outright —
   instead of eight accreted flags (§3).
3. No serialized interchange document at all (§3) — not merely one instead of
   two, which is what an in-place narrowing would have reached for.
4. Value-returning fallible callbacks designed in from the start (§2.1) rather
   than bolted onto an IR that rejects them.
5. No `Json` carrier, by construction (§3). fluessig cannot delete its degrade
   paths without breaking the cross-package consumers that lean on them; a
   restart with a rustc-checked front end never grows them.

If the resulting IR looks materially like `api.rs` with the entity references
removed, the restart did not pay, and that is worth noticing early — at the end
of §7's step 1, not after seven backends have been re-typed.

---

## 7. The first thing that works

**One Rust function, one target language, actually callable.** The first
milestone is **jawohl's `complete_json` in Python** — chosen because it is the
smallest honest end-to-end case (jawohl 1.0 is 58 lines, zero dependencies, pure
`std`, and it builds and passes clean today [verified]) and because it closes a
promise the jawohl README has carried unfulfilled since May 2023: *"soon wrappers
published for Javascript and Python."*

**What the user types:**

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
cargo jedem generate --python    # -> the pyo3 binding
maturin develop
```

**What they get:**

```python
>>> import jawohl
>>> jawohl.complete_json('{"key": "value", "arr": [1, 2, {"nested": "v')
'{"key": "value", "arr": [1, 2, {"nested": "v"}]}'
>>> jawohl.complete_json('{"a": 1}}')
jawohl.MalformedJsonError: The input JSON string is malformed.
```

Note what this exercises with no ceremony: a free function (no handle), a `&str`
param, a fallible `Result<T, E>` return lowering to a real Python exception, and
the synchronous-by-default projection. Nothing here is async, nothing is a
callback, nothing is a stream — which is the point of a first milestone.

**Then, in order, each step adding exactly one of the hard three:**

| Step | Adds | Proves |
|---|---|---|
| 1 | `complete_json` → Python | the spine: derive → descriptor → generator → backend → a real host process |
| 2 | the same function → node/TS | one surface, two languages, no second declaration |
| 3 | `Stream::from_json_schema()` → a handle with `push`/`snapshot` | **handles** (§2.2), incl. the factory mint |
| 4 | `stream.changes()` → `for await` / generator | **streams + events** (§2.3), errors-as-events |
| 5 | a native validator param | **value-returning fallible callbacks** (§2.1) — the new thing |
| 6 | .NET | the new backend |

Steps 3–5 are exactly the hard three, in ascending order of risk, each against a
real consumer rather than a demo. And per `findings.md`'s method: **author
jawohl 2.0's and pidgin's complete surfaces before freezing the IR** — that is
what caught every gap in fluessig, and it is the cheapest insurance jedem can buy.

---

## 8. Does the design live up to its name?

**jedem** — German *to each*, the dative of *jeder*. The name sets a coverage
test, and it is a harder one than the design's previous name asked for: not *is
the translation faithful?* but **does every language actually get it?**

That is the better question, because it is countable. Three things in the design
are failures of it, and the doc already tracks all three:

**Where every language does get it:**

- **The callback contract.** The core sees one `Box<dyn Fn>`; each language
  supplies its own native callable — a JS closure, a Python callable, a Ruby
  `Proc`, a PHP `Zval`. One contract, seven native spellings, nobody left out.
  This is the design's best claim on the name.
- **Idiomatic spelling, not lowest-common-denominator.** A `bytes` param becomes
  `Uint8Array` and a `bytes` return becomes `Buffer`, because that is what a JS
  developer would have written. A sync Rust function becomes a sync JS function
  rather than a `Promise`. Unions become native discriminated unions; streams
  become real `for await` iterables. Each language gets the version *it* would
  have written, not a transliteration of Rust.

**Where some language does not:**

- **Skip-notes are the sharpest failure.** Five of eight backends emit *nothing*
  for a handle mint (§5). Not a poor rendering — an absence. Under this name that
  is the single worst thing in the design, and §7 steps 3 and 6 exist to shrink
  it.
- **The `Json` carrier** hands a language a degraded version: the value crosses,
  the typed methods vanish. **Banned by construction** (§3): no `Json` type in
  the vocabulary, no envelope projection, unlowerable types are compile errors.
  It appears in this list only because the ban must be *kept* — the pressure to
  add "just pass it as JSON for now" will recur, and §3 is the standing answer.
- **`@manual`** means that language got a hand-written binding instead of a given
  one. It earns its keep as an escape hatch, and every use is still one place the
  promise was not kept.
- **PHP's sync-only callbacks** are the one imperfection that cannot be removed:
  the signature is identical but means something weaker, because the runtime
  genuinely differs. Handled correctly — a loud marker, not a silent lie — but the
  name is not fully satisfied there and never will be.

**And the honest one, at v1.** The v1 boundary (§1) ships to **two** languages.
"To each" is an aspiration the first release deliberately does not meet. That is
the right call — two languages done properly beats eight done partially — but the
name is a promise the roadmap owes, not a description of v1.

**The metric the name implies.** Under the old name the health measure was the
count of `Json` carriers and `@manual` ops. Under this one it is **coverage**: for
each backend, what fraction of the declared surface actually lowers, and how many
skip-notes remain. That number is mechanically computable, it should only ever go
up, and it is the one number that says whether the project is living up to what it is called. (It is
computed from the in-memory descriptors — `--dump-surface` shows them — against
each backend's emitted output; there is no surface document, per §3.)

## 9. Decisions and open questions

### Resolved

1. **Repo and name — `PowderworksCode/jedem`, a new repo.** Not a fluessig
   rename. Consequence: pidgin's existing `#[fluessig(...)]` attributes need a
   rename pass to `#[jedem(...)]` when it migrates, and `<Iface>Core` /
   `cargo fluessig emit` / `fluessig-gen` all get renamed spellings.
2. **.NET binding technology — not prescribed.** Whichever Rust→C# bindgen
   actually works, chosen on contact (§1). This is deliberately a §7-step-6
   decision, not a design-time one; no research is owed before then.
3. **jawohl's schema adapters — hand-written per language.** Pydantic → JSON
   Schema, Zod → JSON Schema and friends are ordinary host-language libraries
   sitting *above* jedem's generated surface. jedem does not generate them,
   and they are what keeps "bindings stay thin" true.
4. **MCP — dropped, permanently** (§4), taking `readonly`/`destructive`/`worker`
   with it (§3).

### Open

5. **Does jawohl wait, or get a temporary hand-written binding?** The v1 boundary
   (§1) and jawohl's day-one-consumer status are in tension: only `complete_json`
   is v1-shaped. Either jawohl's useful surface waits for steps 3–5, or a
   throwaway binding bridges the gap. Currently assumed: it waits.
6. **What replaces the entl/disponent parity gates?** They were the standing
   proof that a change did not silently alter output, and they leave with the
   entity graph (§4). pidgin and jawohl should inherit that role, but neither has
   a committed golden yet, and a generator without a parity gate regresses
   quietly.
7. **Does `Foreign` survive v1?** It exists for genuinely external host types
   (`http.Server`, a `ChildProcess`) and lowers to an opaque handle. Neither
   consumer's v1 surface obviously needs one, and §1's boundary argues for cutting
   it until something does.
