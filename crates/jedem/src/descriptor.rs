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
    /// True when the Rust parameter was a reference (`&str`, `&[u8]`).
    ///
    /// [`Type`] says what crosses the boundary; this says how the core wants
    /// to receive it, and the two differ per language. pyo3 can take a
    /// borrowed `&str` straight from the interpreter, while napi hands over an
    /// owned `String` that the call site must re-borrow. Without this the
    /// second backend generates code that does not compile.
    pub borrowed: bool,
}

/// A C-like enum crossing the boundary.
///
/// Only unit variants: an enum carrying data is a union, which is a different
/// feature. Each language gets its own real enum rather than a string —
/// Python a member of an `enum`-like class, TypeScript a string-literal union —
/// so a typo is caught by that language's own tooling instead of surfacing as
/// a value nobody matched.
#[derive(Debug, Clone, Copy)]
pub struct EnumDef {
    /// The Rust type name, reused as the type name in each language.
    pub name: &'static str,
    pub doc: Option<&'static str>,
    pub variants: &'static [Variant],
}

#[derive(Debug, Clone, Copy)]
pub struct Variant {
    /// The Rust variant name.
    pub name: &'static str,
    /// How it is spelled at the boundary. Defaults to the Rust name; pin it
    /// with `#[jedem(name = "...")]` when a wire spelling is already fixed.
    pub wire: &'static str,
    pub doc: Option<&'static str>,
}

/// Implemented by `#[derive(jedem::Enum)]`. The bridge from a Rust type to its
/// descriptor, so a signature can name the type directly.
#[diagnostic::on_unimplemented(
    message = "jedem cannot lower `{Self}`",
    label = "not a type jedem can cross a language boundary",
    note = "v1 handles bool, integers, f64, String/&str, Vec<u8>, Option<T>, Vec<T>, \
            and any enum deriving `jedem::Enum`",
    note = "there is deliberately no fallback that would pass this across as an opaque blob"
)]
pub trait EnumType {
    /// This type's descriptor.
    const DEF: &'static EnumDef;
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
    /// A C-like enum; see [`EnumDef`].
    Enum(&'static EnumDef),
}

impl Type {
    /// Does a value of this type ever cross as absent?
    pub fn is_optional(&self) -> bool {
        matches!(self, Type::Optional(_))
    }

    /// The enum this type reaches, looking through `Option` and `Vec`.
    pub fn enum_def(&self) -> Option<&'static EnumDef> {
        match self {
            Type::Enum(d) => Some(d),
            Type::Optional(inner) | Type::List(inner) => inner.enum_def(),
            _ => None,
        }
    }
}

impl PartialEq for EnumDef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for EnumDef {}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.wire == other.wire
    }
}
impl Eq for Variant {}
