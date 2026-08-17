//! The descriptor: what the macros capture, and the only thing the generator
//! reads.
//!
//! Every field is `&'static`, because the macros expand to **pure data** — no
//! behaviour, no files written at expansion time. A `#[jedem::export]` impl
//! compiles to the impl you wrote, plus a constant describing it.
//!
//! Nothing here is ever serialised. The generator is a library the exporter
//! links and calls on these values directly, so there is no interchange
//! document, no schema to version, and no way for a checked-in artefact to go
//! stale against the code it describes.

/// A whole surface: everything one crate exposes.
#[derive(Debug, Clone, Copy)]
pub struct Surface {
    /// Module/package name in the target language.
    pub name: &'static str,
    pub version: &'static str,
    pub interfaces: &'static [&'static Interface],
}

/// One `#[jedem::export] impl` block.
#[derive(Debug, Clone, Copy)]
pub struct Interface {
    /// The Rust type the impl is on.
    pub name: &'static str,
    pub doc: Option<&'static str>,
    pub ops: &'static [Op],
}

/// One exported function.
#[derive(Debug, Clone, Copy)]
pub struct Op {
    /// The Rust function name.
    pub name: &'static str,
    pub doc: Option<&'static str>,
    /// Exact name to export under, when the author pinned one with
    /// `#[jedem(name = "...")]`. Otherwise each backend applies its own
    /// idiomatic casing.
    pub export_name: Option<&'static str>,
    pub params: &'static [Param],
    /// What the function returns, with any `Result` unwrapped — see
    /// [`Op::fallible`].
    pub returns: Type,
    /// True when the Rust return type is `Result<T, E>`, in which case the
    /// binding gets its language's native error seam. Inferred from the
    /// signature rather than declared: unlike async-ness, the signature
    /// genuinely reveals it.
    pub fallible: bool,
    /// Path to call, relative to the crate root — `Jawohl::complete_json`.
    pub rust_path: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Param {
    pub name: &'static str,
    pub ty: Type,
}

/// The v1 type vocabulary: plain values.
///
/// Deliberately small. A type jedem cannot lower is a compile error at the
/// derive, never a stringly carrier — there is no `Json` escape hatch, so
/// growing this list is the only way to widen what can cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    /// No value; `()` in Rust, `None`/`void`/`null` at the boundary.
    Unit,
    Bool,
    I32,
    I64,
    F64,
    Str,
    /// `Vec<u8>` / `&[u8]`. Spelled position-aware where a language
    /// distinguishes a borrowed view from an owned buffer.
    Bytes,
    Optional(&'static Type),
    List(&'static Type),
}

impl Type {
    /// Does a value of this type ever cross as absent?
    pub fn is_optional(&self) -> bool {
        matches!(self, Type::Optional(_))
    }
}
