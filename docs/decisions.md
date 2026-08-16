# jedem — decision record

The [design](./jedem-design.md) states what jedem is; this file records how it
got that way — each decision with its rationale, the alternatives that were
considered and killed, and the evidence. The fluessig evidence base is in the
[reading notes](./fluessig-reading-notes.md).

Confidence marks as in the design doc: **[verified]** / **[speculation]**.

---

<a id="d1"></a>
## D1. Restart, rather than narrow fluessig in place

**Decision (owner):** jedem is a new project. fluessig's notes are the spec;
fluessig's code is prior art to port from, not a tree to prune.

**The evidence was honestly against it.** The reading found no structural
defect forcing a restart: fluessig's function-exposure half (`src/api.rs` +
`src/bindgen/**`) is already independent of everything jedem drops — `api.rs`
imports only `std` + serde, and the 12,938-line bindgen reaches into the schema
side through exactly two case-conversion helpers [verified]. It was already the
only part under development (last 60 commits: bindgen 87 touches, schema files
zero) [verified]. And fluessig had already survived a total front-end
replacement in place — TypeSpec was built, dogfooded on two consumers, then
deleted [verified].

**So the restart must buy what narrowing could not.** Five things, each now a
design fact:

1. `Handle` as a first-class type ([D5](#d5)) — fluessig's own note declined
   this *only* to keep goldens byte-identical.
2. Kind and projection as the only two op axes ([D2](#d2)) — the effects axis
   deleted outright.
3. No interchange document at all ([D3](#d3)) — narrowing would have merged
   fluessig's two documents into one, not questioned whether either should
   exist.
4. Value-returning fallible callbacks designed in ([D7](#d7)) rather than
   bolted onto an IR that hard-rejects them.
5. No `Json` carrier, by construction ([D4](#d4)) — fluessig cannot delete its
   degrade paths without breaking the cross-package consumers leaning on them.

**Failure condition:** if jedem's IR ends up looking materially like `api.rs`
with the entity references removed, the restart did not pay. Check at the end
of roadmap step 1, not after seven backends are re-typed.

<a id="d2"></a>
## D2. Functions only — and only two op axes

**Decision (owner):** DDL, ORM models, format codecs, the Arrow data plane and
MCP generation are **never**. Not "later" — never. entl and disponent stop
being consumers and own their schemas; the entity graph (`Entity` / `Edge` /
`AbstractRoot` derives, `Id<T>`, polymorphic key enums, column-parity
machinery) dies with them. `ArrowBatch` may still cross the FFI as an ordinary
opaque byte type — that is jedem doing its one job on a type that happens to be
Arrow, not a data plane.

**MCP took an IR axis with it.** fluessig's `readonly` / `destructive` /
`worker` op flags are consumed by `src/bindgen/mcp.rs` and nothing else — zero
references in any other backend [verified]. Dropping MCP (518 lines, a second
product wearing jedem's clothes) deletes the whole semantic-metadata category,
collapsing the op model to kind × projection.

<a id="d3"></a>
## D3. No interchange document

**Decision:** jedem serializes nothing. Derive → descriptor → generator →
bindings, one process.

**Why the document existed:** fluessig's locked-decisions table says it plainly
— *"Language ↔ core interchange — `catalog.json`, versioned, fully resolved —
Rust core never embeds Node"* [verified, fluessig `notes/design.md:113`]. The
front end was TypeSpec, a Node program; the engine was Rust; JSON was how they
spoke.

**Why it must die:** TypeSpec was deleted ~100 commits before jedem was
conceived. With a Rust front end and a Rust generator, a `surface.json` is Rust
serializing to JSON so Rust can immediately parse it back inside one toolchain
— a vestige of a removed boundary. (An early jedem draft proposed merging
fluessig's two documents into one; that was the wrong fix — one vestige instead
of two.)

**What went with it:** a serde round-trip; a schema to version; the
`skip_serializing_if` house style that shaped every flag ever added to
fluessig's `ApiOp`; a loader that re-validates a document Rust just wrote; the
whole "checked-in document is stale" failure class. Much of `api.rs`'s 785
lines is exactly this scaffolding and does not port.

**What replaced its two real jobs:** regression protection moved to goldening
the **generated bindings** (a stronger gate — it catches generator changes
too); debuggability is `cargo jedem generate --dump-surface`, explicitly a
debug artifact and not an interface. Cross-crate surfaces improve outright:
crate B's descriptors are `&'static` items crate A links and rustc checks,
instead of two JSON documents merged by name.

<a id="d4"></a>
## D4. No `Json` carrier

**Decision (owner):** no value crosses the FFI as a JSON blob. Three teeth:

1. **No `Json` type in the vocabulary** — nothing to degrade *to*. An
   unlowerable type is a spanned compile error at the derive.
2. **No envelope union projection.** fluessig's structured-union mode kept a
   `{"kind","payload"}`-string opt-out; that is the carrier by another name,
   and a projection mode is the most expensive surface to keep (every backend
   implements both forever). Structured is the only mode; a backend that cannot
   lower it yet skip-notes. Java crosses unions as envelope strings today, so
   java unions start as a skip-note — absent, not degraded.
3. **`Foreign` is the only opaque crossing**, author-declared at the signature
   for genuinely external host types — never a generator fallback.

**Why fluessig couldn't do this:** it degraded exactly where its front end
could name types it could not see — an unrecognised scalar maps to `String` at
the shared type chokepoint (`ty()`, `src/bindgen/mod.rs:433`; the note's own
words: "the typed methods on that object vanish"), and a cross-package type
resolved without `--context` degrades to a bare `Json` scalar [from fluessig
`notes/class-handle-return.md`]. jedem's front end is rustc: every exported
type is one the compiler already resolved, so "unrecognised" is not a reachable
state.

**The ban must be kept:** the pressure to "just pass it as JSON for now" will
recur. This entry is the standing answer.

<a id="d5"></a>
## D5. `Handle` and `Record` are different types

**Decision:** an op returning a live object and an op returning plain data get
different IR spellings — `Type::Handle(InterfaceId)` vs
`Type::Record(RecordId)` — with reference payloads, constructible only pointing
at declared things.

**What fluessig did and why:** it spelled both as `ApiType::Model { model }`
and told them apart at lowering time by name-membership in the interface set —
chosen, per its own note, so that "existing goldens with DTO-returning ops are
byte-identical." jedem has no goldens; the compat constraint does not exist.
`constructible_interfaces` / `returned_interface_name` stop existing as
concepts, and a `Handle` naming something undeclared is unconstructible rather
than silently becoming a DTO reference.

**Inherited intact from the notes, unchanged:** the factory core-trait shape
(the core returns `Arc<core::<Iface>Impl>`, never the generated class pure Rust
cannot name — the trap the notes document); handle classes for factory-born
interfaces with no public constructor; async methods on handles from day one;
async/stream factories deferred.

<a id="d6"></a>
## D6. Synchronous by default; async is a per-op opt-out

**Decision:** a plain op generates a synchronous binding on every backend;
`#[jedem(async)]` opts out. Declared in exactly one place, same meaning
everywhere, no surface-level default lever.

**History:** fluessig started async-by-default (node wrapped every unary op in
`AsyncTask` → `Promise`) and had to invert it when pidgin's deliberately
sync-and-infallible surface fought the wrapper on every backend. The inversion
is kept, not re-derived. Inferring async-ness from the Rust signature was
considered and rejected: an IO-bound *sync* Rust fn should still be async at
the binding so it does not block the host's event loop — async-ness is a
projection decision the author declares, not a property the signature reveals.
Infallibility, by contrast, *is* inferred (`T` vs `Result<T>`) — the signature
genuinely does reveal it.

<a id="d7"></a>
## D7. Value-returning, fallible callbacks — on synchronous ops only

**Decision:** the uniform callback shape is
`Box<dyn Fn(A) -> Result<R, CallbackError> + Send + Sync>`, legal only on
synchronous ops, invoked re-entrantly on the host thread.

**What fluessig does:** hard-rejects the shape — *"only forward-only sync void
callbacks are supported (is_async/fallible/non-void returns not yet
implemented)"* [verified, `src/api.rs:585`] — correctly for pi, whose
enumerated surface has no value-returning callback anywhere.

**Why jedem must differ:** jawohl's native validators — a host predicate
returning a verdict that can fail — *are* the feature.

**Why the sync-only rule:** calling into a host and waiting for a value is safe
only when the call originates on the host's own thread; from a Rust background
thread it needs an async-oneshot bridge and can deadlock a single-threaded
runtime — the exact reason fluessig deferred it. The restriction is checkable
in the macro (spanned) and re-checked in the validator, and it makes the
feature implementable on every backend including PHP. It is also precisely
jawohl's shape: `push()` is host-called and synchronous. [speculation — the
deadlock analysis is from the notes; no code proves the widened shape yet.]

**Still deferred:** genuinely async callbacks (core awaits a host promise). No
consumer needs one; the two-forward-halves pattern is the workaround.

<a id="d8"></a>
## D8. Derive front end mechanics

Inherited deliberately from fluessig's derive-front-end decisions, re-examined
and kept:

- **Derive → `&'static` descriptor → separate generation step.** Macros never
  write files (non-hermetic, breaks incremental compilation).
- **Explicit `surface!` root list**, reachability for the rest — not
  `inventory`/`linkme` link-sections, which are flaky on wasm.
- **`syn` + `darling`, no reflection substrate.** A substrate could at best
  replace descriptor *capture*, never generation; `facet` is pre-1.0 with
  attributes "in flux", `bevy_reflect` is a runtime system on a game-engine
  release cadence.
- **Spans and docs flow through:** `///` comments and `file!()`/`line!()` land
  in the descriptors so diagnostics point at the author's `.rs` lines.

<a id="d9"></a>
## D9. jawohl is a full jedem consumer from day one

**Decision (owner):** jawohl 2.0 writes no bindings by hand and waits for
jedem's capability steps.

**The cost, on the record:** nothing useful in jawohl fits jedem's v1 boundary
— `Stream` is a handle mint, `changes()` a stream, native validators callbacks;
only `complete_json` is v1-shaped. jawohl's surface track is gated end to end
on roadmap steps 3–5, and its fourth language (.NET) on an entire new backend.
Two things make the bet good: jawohl's Rust core — the majority of its
engineering — is not blocked at all, and jawohl's surface is the best available
acid test, small and precise and hitting all three hard cases. The release
valve, if the schedule bites: a throwaway hand-written pyo3 binding for two
free functions.

<a id="d10"></a>
## D10. Smaller resolved decisions

- **Name and home:** **jedem** (jedem.dev), `PowderworksCode/jedem`, a new
  repo. Consequences: pidgin's `#[fluessig(...)]` attributes need a rename pass
  on migration; `<Iface>Core` respells; fluessig's two-step `emit`/`fluessig-gen`
  CLI is not renamed but **replaced** by one-step `cargo jedem generate`.
  (Marketing note: avoid the phrase "Jedem das Seine" — the Buchenwald-gate
  association makes it a live third rail in Germany; the bare word is fine.)
- **.NET binding technology: unprescribed.** Whichever Rust→C# bindgen works —
  `csbindgen`, `interoptopus`, or a hand-rolled C ABI + P/Invoke — chosen on
  contact at roadmap step 6. No research owed before then.
- **jawohl's schema adapters are hand-written per language** (Pydantic → JSON
  Schema, Zod → JSON Schema), sitting above jedem's generated surface. jedem
  does not generate them; they are what keeps "bindings stay thin" true.
- **jawohl moves to Powderworks by transfer**, owner-handled. Nothing in these
  designs touches `genauai/jawohl`.
- **Runnable proofs are the done-bar:** every backend keeps a CI'd host-process
  round-trip, inherited from fluessig's demo-crate discipline.
