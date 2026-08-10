# calquer — reading notes (pre-design)

Status: reading only. No design here. No commits/PRs/pushes made in either repo.
Everything under "Verified" was run or read directly; "Inferred" is my reasoning
on top of it.

---

## 1. Why is fluessig being restarted rather than narrowed?

**I could not find a structural reason. The evidence points the other way: the
function-exposure half of fluessig is already a cleanly separated subsystem, and
it is already the only part under active development.**

### Verified — the coupling is nearly zero

| Fact | Evidence |
|---|---|
| `src/api.rs` (785 ln) imports **only** `std::collections::BTreeMap` + `serde` | `grep '^use ' src/api.rs` |
| `src/bindgen/**` (12,938 ln) imports from the schema side **only** `ir::{camel, snake}` — two case-conversion helpers, 5 call sites | `grep -ro 'ir::[A-Za-z]*' src/bindgen/` → `camel` ×4, `snake` ×1 |
| `grep -rn 'use crate::(ir\|catalog)' src/bindgen/` → **no matches** | — |
| `bindgen/mod.rs` pulls exactly `crate::api::{ApiDoc, ApiOp, ApiType, ApiUnion, ForeignType, Shape}` | `src/bindgen/mod.rs:53` |

So `api.rs` + `bindgen/` = **13,723 lines that already form a standalone
function-exposure system** with no dependency on the entity graph, DDL, ORM,
codecs, or Arrow.

**The one real seam:** `src/bin/fluessig-gen.rs` takes `catalog.json` as a
*required positional* arg, and extracts the enum vocabulary from
`catalog.enums` into `Vec<bindgen::EnumDesc>` to feed the backends
(`fluessig-gen.rs:132, 199`). `--api api.json` is a flag on top. That is the
only thing forcing a catalog to exist for a bindings-only build — a
one-file plumbing change (make the catalog optional, or move enums into
`api.json`), not a rewrite.

### Verified — fluessig is *already* calquer, by commit history

- Last 60 commits (2026-07-21 → 2026-08-08) touched:
  `src/bindgen/*` **87** times, `src/api.rs` **8** times,
  `src/{ir,catalog,sql,codegen,data,observe}.rs` **0** times.
- Last commit touching that schema set at all: **2026-07-20**, `5525299`,
  and it was a lint-suppression chore. **80 commits ago**, out of 219 total.
- The entire `CHANGELOG.md` `[Unreleased] / Added` section is binding work
  (Java JNI backend, node stream error model, `@streamError`).
- The named consumer driving all recent work is **pidgin** (formerly atilla) —
  "~157 symbols that are almost entirely fluessig-generatable" — a *binding
  surface*, not a schema. The sync-by-default inversion, op export-name pins,
  binary `Uint8Array`/`Buffer` spelling, the `{ok,value}|{ok,error}` envelope
  and `single_threaded` all exist to serve it.

### Verified — fluessig has already survived one front-end replacement in place

`design.md` carries a `> [!NOTE] **Superseded.**` banner. The TypeSpec front end
(`@fluessig/emitter`, `@fluessig/typespec`, all `.tsp` sources, Node) was
**built, dogfooded on two consumers, then deleted**, and replaced by
`#[derive(Entity)]` — with `catalog.json`/`api.json` unchanged.
`derive-front-end-decisions.md` §2 records it as DONE. That is a precedent that
this codebase can absorb a total front-end swap without a restart, and it is the
same magnitude of change calquer needs.

### The honest case *for* restarting

Not structural, but real, and the user should weigh it:

1. **Naming and positioning.** "fluessig" is stamped on the crate, the README
   ("Describe a typed entity graph once"), `#[fluessig(...)]` in every consumer's
   source, `<Iface>Core` traits, `cargo fluessig emit`, `fluessig-gen`. Renaming
   is mechanical but touches every downstream consumer's source.
2. **The derive front end carries entity baggage.** `fluessig-derive` +
   `fluessig-derive-macros` = 4,916 lines exporting 7 derives; **`Entity`,
   `AbstractRoot`, `Edge`** (plus `Id<T>`, `ref_cols`, `shares`, `flatten`,
   polymorphic key-enum generation, `EntityDescriptor`/`EdgeDescriptor`) are
   entity-graph-only. calquer needs `Record`, `Union`, `Enum`, `Scalar`,
   `catalog!` and `#[fluessig::export]`. Roughly half this crate pair is dead
   weight for calquer — but it is *deletable* weight, not entangling weight.
3. **The dogfood consumers are schemas.** `entl-schema-derive` (1,212 ln) and
   `disponent-schema-derive` (1,098 ln) exist purely as entity-graph parity
   gates. Narrowing means deciding whether to keep supporting them.
4. **Doc rot as a signal.** The CHANGELOG still calls `@streamError` a "TypeSpec
   decorator" months after TypeSpec was deleted. The stated identity and the
   actual work have visibly diverged.

### My read (inferred)

Every one of those four is a **deletion-and-rename** problem, not an
architecture problem. The thing a restart usually buys — escaping a substrate
that fights the new scope — is not on offer here, because `api.rs` never
depended on that substrate in the first place. A restart would begin by
re-typing 13,723 lines of working, seven-backend, test-covered lowering code
that has no known structural defect.

**My recommendation is narrow in place** (delete `ir/catalog/sql/codegen/data`,
delete the `Entity`/`Edge`/`AbstractRoot` derives, cut the catalog seam in
`fluessig-gen`, rename) — *unless* the user is optimising for something the code
can't tell me: a clean public repo, a licensing/ownership boundary, dropping the
entl/disponent obligations, or simply wanting to re-decide the op IR from
scratch. Those are legitimate and I can't adjudicate them from the tree.

**This is a genuine fork and I am asking rather than assuming.**

---

## 2. What in the notes is load-bearing for calquer

Ranked. The first three are the brief's "hard three", and the headline is that
**fluessig has already solved them** — the notes are not a research plan, they
are an implementation record with tests.

### (a) `callback-function-types.md` — LOAD-BEARING, largely SOLVED

- The contract: **the Rust core sees one uniform shape regardless of source
  language** — `Box<dyn Fn(A, B) + Send + Sync + 'static>`. Each backend's
  *generated* glue wraps its native callable into that box at the FFI boundary.
  This is the single most reusable decision in the notes.
- Two IR additions, both additive and byte-compatible with existing goldens:
  `ApiType::Callback { params, returns }` and `Shape::Subscription` (one callback
  param → a handle whose drop/`unsubscribe()` deregisters).
- **Deliberately narrow scope, justified by evidence:** a source-level
  enumeration of pi found *every* callback is forward-only, sync, void-returning,
  single-arg. `is_async` / `fallible` are reserved in `CallbackSig` and
  **rejected by `load_api`**. Duplex is modelled as two forward-only halves plus
  external correlation. calquer should inherit this restriction verbatim; it is
  what keeps callbacks from requiring an async-oneshot bridge.
- **Per-backend non-blocking table** (node TSFN `NonBlocking`, python
  `with_gil`, ruby GVL trampoline, java `AttachCurrentThread`, wasm keep-`Closure`-alive,
  cpp `std::function`, php `Zval`). This table is the expensive learning.
- **PHP is the honest edge:** single-thread request model means callbacks are
  **sync-same-request-thread only**; off-thread invocation is UB. Ruled
  documented-limitation, not a hard error. calquer must decide the same thing.
- Status: all 7 backends lower `Callback` + `Subscription` (PRs #87–#91).
  Tests: `callback_lowering.rs`, `{wasm,ruby,cpp,php,java}_callback_lowering.rs`,
  `subscription_lowering.rs`.

### (b) `class-handle-return.md` — LOAD-BEARING, PARTLY SOLVED

- **Key decision: no new IR vocabulary.** An op returning a handle is spelled
  `{"model": "<Iface>"}` — the same as a DTO — and disambiguated *at lowering
  time* by membership in the interface-name set (`returned_interface_name`,
  which unwraps `Nullable`/`List`). Existing goldens stay byte-identical.
- **The actual crux, and the non-obvious part:** the core trait must return the
  **core object**, `anyhow::Result<Arc<crate::core_impl::<Iface>Impl>>`, *not*
  the generated handle class — because the pure-Rust core cannot name or build a
  napi/pyo3 class. The binding then wraps: `Ok(Iface { core: … })`. Getting this
  backwards is the trap, and the note documents it as a discovered gap.
- **Handle-class emission gate moves from `has_ctor` → `constructible`**
  (`has_ctor || returned_somewhere`), so a factory-born interface gets a class
  with methods and *no* public constructor.
- Async methods **on** the returned handle: supported day one (rides existing
  machinery). An async or stream **factory op** (the mint itself): deferred,
  skip-note.
- Status: node + python lower fully; **cpp/java/ruby/php/wasm emit honest
  skip-notes**. Test: `tests/class_handle_return.rs`. This is the *least*
  finished of the hard three and the most likely real work item for calquer.

### (c) `async-iterable-streams.md` (+ `-python`, `-ruby`) — LOAD-BEARING, SOLVED for node

- Node: genuine `for await` via napi 3 `#[napi(async_iterator)]` +
  `impl AsyncGenerator`, with `next()` retained as a feature-independent poll
  fallback. Core primitive is a **blocking** `PollStream::poll(&self, timeout)`
  driven through `napi::tokio::task::spawn_blocking` so the event loop stays
  free. Backpressure is by protocol (napi pulls one at a time). Cancellation via
  `AsyncGenerator::complete` → `PollStream::close()`, with `impl Drop` as backstop;
  `close()` must be idempotent.
- **Risk flagged in the note itself:** `#[napi(async_iterator)]` is
  **experimental** in napi 3 and gated on `tokio_rt`. calquer inherits that
  exposure. The retained poll cursor is the hedge.
- **Dual error model** — the sharpest reusable decision: *streams* use
  errors-as-events (a terminal error event then completion, `next()` never
  rejects) while *unary/ctor* keep thrown errors; construction-time errors always
  throw. Per-op opt-in via `@streamError`; node's default is now
  reject-the-pull (safe-by-default), with error-as-event as mirror-a-library mode.

### (d) `derive-front-end.md` + `derive-front-end-decisions.md` — LOAD-BEARING for *how calquer is authored*

- **"The impl is the interface."** `#[fluessig::export] impl Foo { … }` derives
  `api.json` from the code that actually runs, so declaration/implementation
  drift is impossible. This is the uniffi consumption model and it is exactly
  calquer's thesis. Op kinds: `ctor` / unary / `stream` / `manual`, plus flags
  `async`, `readonly`, `destructive`, `result`, `name = "…"`, and the
  interface-level `single_threaded`.
- **Synchronous is the GLOBAL DEFAULT; `#[fluessig(async)]` is the opt-out.**
  This was an *inversion* forced by pidgin, and it is a decision calquer should
  simply inherit — async-ness is decided in exactly one place, per op, meaning
  the same thing on every backend. There is deliberately no catalog-level lever.
- **Infallibility is inferred from the Rust return type** (`T` vs `Result<T>`),
  and propagates all the way into the shared core trait. Ruby is the honest edge:
  its arg marshalling is itself fallible, so a true no-raise `-> T` is emitted
  only for zero-marshalling ops.
- **Derive → `&'static` descriptor → separate exporter.** Macros never write
  files. Reachability from an explicit `catalog!` root list, *not*
  `inventory`/`linkme` link-section magic (flaky on wasm). Plus a
  regenerate-validate-diff `#[test]` drift guard.
- **`syn` + `darling`, no reflection substrate.** `facet` is pre-1.0 with
  attributes "in flux"; `bevy_reflect` is a runtime system. Reasoning holds for
  calquer unchanged.
- Also load-bearing and easy to miss: `single_threaded` (a thread-confined
  `!Send` core, **node-only**, other backends emit nothing + an explicit
  skip-note rather than a `Send`-assuming handle that breaks the consumer's
  build) and the **fail-loud-not-silent** discipline generally — spanned compile
  errors with verbatim messages, re-checked at the loader.

### (e) `findings.md` — mostly NOT load-bearing for calquer

It is the entity-graph acid test (28 entl tables, FK-in-PK, polymorphic families,
column-order parity, `@defaultValue`). Almost all of it dies with the entity
graph. Three things survive:

- The **op-surface findings** section: the four shapes held against a real
  surface; the bindgen type surface needs options-bag models, enums as op types,
  list returns, list-typed fields, and **optional params** (the note flags that
  the extractor dropped param optionality — a real emitter must keep it).
- The **method**: author the complete real surface before freezing the IR. That
  is what caught every gap, and calquer should repeat it against pidgin.
- The `@manual` escape hatch earning its keep (`watch`, host-callback re-entry).

### (f) `java-backend.md`, `specter-navigators.md`, `sampo.md`, `plan.txt`, `entl_derive_sketch.rs` — not yet read

`java-backend.md` is likely relevant (JNI-vs-Panama rationale is summarised in
the CHANGELOG). `entl_derive_sketch.rs` is the entity-graph acid test and is
almost certainly irrelevant to calquer.

---

## 3. Does jawohl still build? — YES, verified

Toolchain: `rustc 1.96.1 (31fca3adb 2026-06-26)` / `cargo 1.96.1`.

| Check | Result |
|---|---|
| `cargo build` | **clean**, 0.44s |
| `cargo test` | **2 passed, 0 failed** (`test_complete_json`, `test_malformed_json`) |
| `cargo clippy --all-targets` | **clean — zero warnings** |
| `examples/openai_streaming_parse` (separate workspace, 2023-pinned `async-openai 0.10.3`, `reqwest 0.11.17`, `hyper 0.14.26`) | **builds clean**, exit 0 |

Nothing rotted. Explanation: `[dependencies]` is **empty** — jawohl is pure
`std`, 58 lines, edition 2021 (still current). There is no surface for bit-rot to
attack. Even the 2023 example resolves from its committed `Cargo.lock`.

### Rot that *is* present (not build-breaking)

- **No CI at all** — no `.github/` directory. The last commit's own message reads
  *"~Setting up Python module~ and some CI/CD stuff EDIT: gave up, just bumping
  verison number"*, so CI was attempted and abandoned.
- **`bench.rs` (113 ln) is orphaned** — sits at repo root with no `[[bench]]`
  target in `Cargo.toml`; never compiled, never run.
- **Version is `0.1.1-dev`** despite commits titled "doing a 1.0 now" and
  "Bumping to 0.2.0". Not published to crates.io as far as the manifest shows,
  yet the README instructs `jawohl = "0.1.0"`.
- Metadata points at a wound-down org: `authors = ["Zack Maril <zack@genau.ai>"]`,
  `documentation = "https://genau.ai"`, `homepage/repository = genauai/jawohl`.
- Test coverage is thin: **2 tests, 41 lines**, for a function with real edge
  cases (escapes, nesting, mismatched closers).
- The example's `Cargo.lock` is **stale**: it pins `jawohl 0.1.0-dev` while the
  crate manifest says `0.1.1-dev`. Also lockfile format v3 — modern cargo rewrites
  it to v4 on first build. (Observed and reverted; both repos left untouched.)

### The Python / JavaScript story: **there is none**

Verified — `src/` is 100% Rust, there is no `pyo3`, `napi`, `maturin`, `wasm`, or
`package.json` anywhere in the tree. The README's Features list says
*"— _soon_ wrappers published for Javascript and Python"*, and the motivation
section says Rust was chosen *"so that it could be used in other languages like
Python and Javascript."* The only commit that tried is the last one, which says
it gave up. **The three-language claim is aspirational and was never
implemented.**

Note (not acted on): jawohl lives in `genauai`, not `PowderworksCode`. Adoption
is a transfer-or-fork decision and it is the user's.

### Inferred

jawohl is 58 lines of dependency-free `std` Rust. "Redesign and polish" here is
close to a rewrite-in-an-afternoon, and the interesting questions are all
behavioural (what *should* a partial-JSON completer do at the edges?) rather than
structural. It is also — and this is the connection worth flagging — a perfect
**calquer dogfood**: two pure functions, `&str → Result<String, E>`, no state, no
callbacks, no streams, no handles. It is very nearly the minimal case of
"expose a Rust function everywhere", and the README's unfulfilled Python/JS
promise is exactly the thing calquer exists to deliver.

Awaiting the jawohl design doc before any design work.
