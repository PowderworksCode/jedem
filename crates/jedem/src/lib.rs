//! # jedem
//!
//! Expose a Rust function once; call it from every language, with its shape
//! intact.
//!
//! *jedem* is German for **to each** — one Rust function, handed to each
//! language in its own idiom. A sync function stays sync, an error raises
//! natively, a name reads the way that language would have spelled it.
//!
//! ## How it works
//!
//! Annotate ordinary Rust and name it in a surface. An `impl` block, a `mod`,
//! or a single `fn` — whichever suits the code you already have:
//!
//! ```
//! pub struct Greeter;
//!
//! #[jedem::export]
//! impl Greeter {
//!     /// Greet someone by name.
//!     pub fn greet(name: &str) -> String {
//!         format!("Hello, {name}!")
//!     }
//! }
//!
//! jedem::surface! { name: "hello", version: "0.1.0", api: [Greeter] }
//! ```
//!
//! A crate exporting free functions needs no type to hang them off:
//!
//! ```
//! /// Greet someone by name.
//! #[jedem::export]
//! pub fn greet(name: &str) -> String {
//!     format!("Hello, {name}!")
//! }
//!
//! jedem::surface! { name: "hello", version: "0.1.0", api: [greet] }
//! ```
//!
//! The macros expand to the impl you wrote plus a `&'static` [`Surface`]
//! describing it. A small bin target then hands that constant to
//! [`generate`] and writes the bindings:
//!
//! ```no_run
//! # const JEDEM_SURFACE: &jedem::Surface = &jedem::Surface {
//! #     name: "hello", version: "0.1.0", interfaces: &[] };
//! # fn main() -> std::io::Result<()> {
//! let code = jedem::generate(JEDEM_SURFACE, jedem::Target::Python, "hello");
//! std::fs::write("src/generated.rs", code)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Nothing is serialised
//!
//! There is no interchange document, no schema to version, and no checked-in
//! artefact that can go stale against the code it describes. The generator is
//! a library, called on the descriptors directly, in one process.
//!
//! ## What v1 covers
//!
//! Functions that take and return **plain values**: `bool`, integers, `f64`,
//! `String`/`&str`, `Vec<u8>`, `Option<T>`, `Vec<T>`, and any C-like enum
//! deriving [`Enum`]; synchronous; fallible or not — with any error type that
//! implements `Display`, including `Box<dyn Error>`. Callbacks, handles to stateful objects, and streams are designed but
//! not in v1 — see `DESIGN.md`.
//!
//! A type jedem cannot lower is a **compile error at the macro**, never an
//! opaque blob smuggled across as a string.

mod descriptor;
mod gen;

pub use descriptor::{EnumDef, EnumType, Interface, Op, Param, Surface, Type, Variant};
pub use gen::{generate, generate_crate, GeneratedFile, Target};

/// Mark functions for export — on an `impl` block, a `mod`, or a bare `fn`.
/// See the [crate docs](crate).
pub use jedem_macros::export;

/// Declare a crate's surface. See the [crate docs](crate).
pub use jedem_macros::surface;

/// Let a C-like enum cross a language boundary.
///
/// Each language gets its own real enum — a class in Python, a string-literal
/// union in TypeScript — rather than a bare string that only conventions keep
/// correct.
///
/// ```
/// #[derive(jedem::Enum)]
/// pub enum Ripeness {
///     Missing,
///     Partial,
///     Done,
/// }
/// ```
pub use jedem_macros::Enum;

/// Write every binding crate, from one line in a bin target.
///
/// jedem cannot read a `&'static` surface without running the crate that
/// contains it, so generation is a bin target rather than a pure cargo
/// subcommand — `cargo jedem generate` runs this. What the macro removes is the
/// identical `main()` every consumer was otherwise writing.
///
/// ```ignore
/// // src/bin/jedem-generate.rs
/// jedem::generator_main! {
///     surface: my_surface::JEDEM_SURFACE,
///     core: "my_surface",
///     out: "..",           // relative to CARGO_MANIFEST_DIR
/// }
/// ```
///
/// Each target gets a directory under `out` named after it, containing a
/// complete crate. Every file carries an `@generated` marker.
#[macro_export]
macro_rules! generator_main {
    (surface: $surface:expr, core: $core:literal, out: $out:literal $(,)?) => {
        $crate::generator_main! {
            surface: $surface, core: $core, core_dir: concat!("../", $core), out: $out
        }
    };
    (surface: $surface:expr, core: $core:literal, core_dir: $dir:expr, out: $out:literal $(,)?) => {
        fn main() -> ::std::io::Result<()> {
            $crate::__write_all(
                $surface,
                $core,
                $dir,
                concat!(env!("CARGO_MANIFEST_DIR"), "/", $out),
            )
        }
    };
}

/// The body of [`generator_main!`]. Public so the macro can reach it; not part
/// of the supported surface.
#[doc(hidden)]
pub fn __write_all(
    surface: &Surface,
    core: &str,
    core_dir: &str,
    out_dir: &str,
) -> std::io::Result<()> {
    for &target in Target::ALL {
        let dir = std::path::Path::new(out_dir).join(target.dir_name());
        let package = format!("{}-{}", surface.name, target.dir_name());
        for file in generate_crate(surface, target, core, core_dir, &package) {
            let path = dir.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &file.contents)?;
            println!("  {}", path.display());
        }
        println!("{} -> {}", target.label(), dir.display());
    }
    Ok(())
}
