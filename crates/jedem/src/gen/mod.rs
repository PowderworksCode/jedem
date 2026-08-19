//! Code generation: descriptors in, a language binding out.
//!
//! Each backend is deliberately small and independent. There is no shared
//! lowest-common-denominator spelling that every language then decorates — the
//! point is that each language gets what *it* would have written, so the
//! per-language differences are the product rather than an inconvenience.

mod node;
mod python;

use crate::descriptor::{Surface, Type};

/// A language to generate for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
    /// A pyo3 extension module.
    Python,
    /// A napi-rs native addon, consumed from TypeScript or JavaScript.
    Node,
}

impl Target {
    /// Every target jedem can generate for.
    ///
    /// Exposed so callers — and jedem's own drift guard — can assert they have
    /// covered all of them, rather than silently missing one added later.
    pub const ALL: &'static [Target] = &[Target::Python, Target::Node];

    /// The conventional file name for this target's binding source.
    pub fn default_file_name(self) -> &'static str {
        match self {
            Target::Python | Target::Node => "generated.rs",
        }
    }

    /// The conventional directory name for this target's binding crate.
    pub fn dir_name(self) -> &'static str {
        match self {
            Target::Python => "python",
            Target::Node => "node",
        }
    }

    /// Human name, for messages.
    pub fn label(self) -> &'static str {
        match self {
            Target::Python => "python",
            Target::Node => "node",
        }
    }
}

/// Generate a binding.
///
/// The output is Rust source for the FFI layer of the target — for Python, a
/// pyo3 module. It is meant to be **committed**, so a reviewer can read what
/// crosses the boundary and a diff shows when it changes.
/// `core_path` is how the generated binding names the crate holding the
/// exported functions: `"crate"` when the binding lives in the same crate, or
/// the crate's name when it is a separate binding crate (the usual shape — a
/// `foo` crate and a `foo-py` beside it).
pub fn generate(surface: &Surface, target: Target, core_path: &str) -> String {
    let body = match target {
        Target::Python => python::generate(surface, core_path),
        Target::Node => node::generate(surface, core_path),
    };
    normalise(&body)
}

/// Make generated output rustfmt-stable, centrally.
///
/// Generated files are committed and diffed against a fresh generation, so
/// anything `cargo fmt` rewrites breaks every build that runs it. Each backend
/// getting this right independently is a bug waiting to recur -- and it has
/// recurred, three times: a trailing space on an empty doc line, a trailing
/// blank line at end of file, and a double blank line between interfaces.
///
/// So the invariants live here rather than in any backend: no trailing
/// whitespace, no run of blank lines, exactly one terminal newline.
fn normalise(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut blank_run = 0usize;
    for line in body.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    let mut out = out.trim_end().to_string();
    out.push('\n');
    out
}

/// `snake_case` -> `camelCase`, for the languages that spell names that way.
pub(crate) fn lower_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = false;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// One file jedem produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// Path relative to the binding crate's root.
    pub path: String,
    pub contents: String,
}

/// Everything a binding crate needs: manifest, module shim, build script where
/// the target requires one, and the binding source itself.
///
/// Emitting only the binding source left every consumer hand-writing a
/// `Cargo.toml`, a module shim and (for node) a `build.rs` — roughly twenty
/// lines per language, none of it about their crate. For a tool whose value is
/// breadth, that cost grew with exactly the thing it was trying to maximise.
///
/// `core_crate` is the crate holding the exported functions; `package` is the
/// name to give the generated binding crate.
pub fn generate_crate(
    surface: &Surface,
    target: Target,
    core_crate: &str,
    core_dir: &str,
    package: &str,
    core_package: &str,
) -> Vec<GeneratedFile> {
    let mut files = match target {
        Target::Python => python::scaffold(surface, core_dir, package, core_package),
        Target::Node => node::scaffold(surface, core_dir, package, core_package),
    };
    files.push(GeneratedFile {
        path: format!("src/{}", target.default_file_name()),
        contents: generate(surface, target, core_crate),
    });
    files
}

/// The `@generated` marker, in a given comment syntax.
///
/// The literal `@generated` is the convention GitHub and most review tools
/// recognise to collapse a file's diff. Every file jedem writes carries it, so
/// nobody has to guess which are hand-written.
pub(crate) fn generated_marker(comment: &str, surface: &Surface, target: &str) -> String {
    let mut out = String::new();
    out.push_str(comment);
    out.push_str(" @generated by jedem for the `");
    out.push_str(target);
    out.push_str("` target. Do not edit.\n");
    out.push_str(comment);
    out.push_str(" Surface `");
    out.push_str(surface.name);
    out.push_str("` v");
    out.push_str(surface.version);
    out.push_str(". Regenerate with `cargo jedem generate`.\n");
    out
}

/// The suffix that converts a value between the core's type and the binding's.
///
/// `From<A> for B` does not give `Option<A> -> Option<B>`, so a container has to
/// be mapped through. Returns `None` when the types are already the same, which
/// is every type except an enum.
pub(crate) fn convert(ty: &Type) -> Option<String> {
    match ty {
        Type::Enum(_) => Some(".into()".into()),
        Type::Optional(inner) => convert(inner).map(|c| format!(".map(|v| v{c})")),
        Type::List(inner) => {
            convert(inner).map(|c| format!(".into_iter().map(|v| v{c}).collect()"))
        }
        _ => None,
    }
}

/// Every distinct enum the surface's ops reach, in first-appearance order.
///
/// Collected here rather than listed in `surface!` because the macro cannot
/// resolve types: it records `Type::Enum(..)` inline where a signature named
/// one, and the backend gathers them. That also means an enum used only inside
/// an `Option` or a `Vec` is still declared.
pub(crate) fn enums_in(surface: &Surface) -> Vec<&'static crate::descriptor::EnumDef> {
    let mut seen: Vec<&'static crate::descriptor::EnumDef> = Vec::new();
    let mut push = |d: Option<&'static crate::descriptor::EnumDef>| {
        if let Some(d) = d {
            if !seen.iter().any(|s| s.name == d.name) {
                seen.push(d);
            }
        }
    };
    for iface in surface.interfaces {
        for op in iface.ops {
            for p in op.params {
                push(p.ty.enum_def());
            }
            push(op.returns.enum_def());
        }
    }
    seen
}

/// The banner every generated file carries.
pub(crate) fn banner(surface: &Surface, target: &str) -> String {
    format!(
        "//! @generated by jedem for the `{target}` target. Do not edit.\n//!\n//! Surface `{}` v{}. Regenerate rather than editing: the source of truth\n//! is the `#[jedem::export]` impl this was derived from.\n//!\n//! Regenerate with `cargo jedem generate`.\n",
        surface.name, surface.version
    )
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::descriptor::{Interface, Op, OpKind, Param, Surface, Type};

    /// A surface exercising a doc comment with a blank line, a pinned export
    /// name, a fallible op and a borrowed param.
    pub const GREET: Op = Op {
        kind: OpKind::Function,
        name: "greet",
        doc: Some("Greet.\n\nWith a blank line, which is where trailing\nwhitespace creeps in."),
        export_name: None,
        params: &[Param {
            name: "name",
            ty: Type::Str,
            borrowed: true,
        }],
        returns: Type::Str,
        fallible: false,
        rust_path: "Hello::greet",
    };

    pub const FALLIBLE: Op = Op {
        kind: OpKind::Function,
        name: "checked",
        doc: None,
        export_name: Some("checked_alias"),
        params: &[],
        returns: Type::Unit,
        fallible: true,
        rust_path: "Hello::checked",
    };

    pub const IFACE: Interface = Interface {
        name: "Hello",
        doc: None,
        ops: &[GREET, FALLIBLE],
        handle: false,
    };
    /// A handle, whose generated constructor is the construct where the
    /// generator most easily drifts from rustfmt: a struct literal long enough
    /// to pass `struct_lit_width` gets broken across lines.
    pub const CTOR: Op = Op {
        kind: OpKind::Ctor,
        name: "new",
        doc: Some("Start at zero."),
        export_name: None,
        params: &[],
        returns: Type::Unit,
        fallible: false,
        rust_path: "Counter::new",
    };

    pub const FALLIBLE_CTOR: Op = Op {
        kind: OpKind::Ctor,
        name: "starting_at",
        doc: None,
        export_name: None,
        params: &[Param {
            name: "start",
            ty: Type::I64,
            borrowed: false,
        }],
        returns: Type::Unit,
        fallible: true,
        rust_path: "Counter::starting_at",
    };

    pub const BUMP: Op = Op {
        kind: OpKind::Method { mutable: true },
        name: "add",
        doc: None,
        export_name: None,
        params: &[Param {
            name: "n",
            ty: Type::I64,
            borrowed: false,
        }],
        returns: Type::Unit,
        fallible: false,
        rust_path: "Counter::add",
    };

    pub const HANDLE: Interface = Interface {
        name: "Counter",
        doc: Some("A live counter."),
        ops: &[CTOR, FALLIBLE_CTOR, BUMP],
        handle: true,
    };

    pub const SURFACE: Surface = Surface {
        name: "demo",
        version: "9.9.9",
        interfaces: &[&IFACE, &HANDLE],
    };
}

#[cfg(test)]
mod tests {
    use super::{generate, Target};
    use crate::descriptor::{Interface, Op, OpKind, Param, Surface, Type};

    const GREET: Op = Op {
        kind: OpKind::Function,
        name: "greet",
        doc: Some("Greet."),
        export_name: None,
        params: &[Param {
            name: "name",
            ty: Type::Str,
            borrowed: true,
        }],
        returns: Type::Str,
        fallible: false,
        rust_path: "Hello::greet",
    };

    const FALLIBLE: Op = Op {
        kind: OpKind::Function,
        name: "checked",
        doc: None,
        export_name: Some("checked_alias"),
        params: &[],
        returns: Type::Unit,
        fallible: true,
        rust_path: "Hello::checked",
    };

    const IFACE: Interface = Interface {
        name: "Hello",
        doc: None,
        ops: &[GREET, FALLIBLE],
        handle: false,
    };
    const SURFACE: Surface = Surface {
        name: "demo",
        version: "9.9.9",
        interfaces: &[&IFACE],
    };

    fn py() -> String {
        generate(&SURFACE, Target::Python, "mycore")
    }

    #[test]
    fn the_banner_names_the_surface_and_forbids_editing() {
        let out = py();
        // `@generated` is the literal token review tools look for; the banner
        // carries it, not merely the word "generated".
        assert!(out.contains("@generated by jedem"), "{out}");
        assert!(out.contains("Do not edit"));
        assert!(out.contains("`demo` v9.9.9"));
        assert!(
            out.contains("cargo jedem generate"),
            "say how to regenerate"
        );
    }

    #[test]
    fn a_borrowed_param_stays_borrowed_and_a_return_is_owned() {
        let out = py();
        assert!(out.contains("pub fn greet(name: &str) -> String"), "{out}");
    }

    #[test]
    fn the_call_targets_the_named_core_crate() {
        assert!(py().contains("mycore::Hello::greet(name)"));
    }

    #[test]
    fn an_infallible_op_has_no_error_seam() {
        let out = py();
        let body = out.split("pub fn greet").nth(1).unwrap();
        assert!(
            !body[..80].contains("PyResult"),
            "infallible must not be wrapped"
        );
        assert!(!body[..80].contains("map_err"));
    }

    #[test]
    fn a_fallible_op_raises() {
        let out = py();
        assert!(out.contains("-> PyResult<()>"));
        assert!(out.contains(".map_err(err)?"));
    }

    #[test]
    fn a_pinned_name_is_emitted_and_registered() {
        let out = py();
        assert!(out.contains(r#"#[pyo3(name = "checked_alias")]"#));
        assert!(out.contains("pub fn checked_alias("));
        assert!(out.contains("wrap_pyfunction!(checked_alias, m)"));
    }

    #[test]
    fn docs_become_docstrings() {
        assert!(py().contains("/// Greet."));
    }

    #[test]
    fn every_op_is_registered() {
        let out = py();
        for n in ["greet", "checked_alias"] {
            assert!(
                out.contains(&format!("wrap_pyfunction!({n}, m)")),
                "{n} missing"
            );
        }
    }
}

#[cfg(test)]
mod format_stability {
    use super::tests_support::SURFACE;
    use super::{generate, Target};

    /// Generated code must survive `cargo fmt` untouched, for **every**
    /// backend.
    ///
    /// It is committed and diffed against a fresh generation, so a formatter
    /// that rewrites it breaks every build. This has already happened twice:
    /// a trailing space on an empty doc line, and a trailing blank line at end
    /// of file. Checking each backend separately is how the second one got
    /// through, so the check iterates `Target::ALL`.
    fn each_target(check: impl Fn(Target, &str)) {
        for &t in Target::ALL {
            check(t, &generate(&SURFACE, t, "mycore"));
        }
    }

    #[test]
    fn no_trailing_whitespace() {
        each_target(|t, out| {
            for (i, line) in out.lines().enumerate() {
                assert_eq!(
                    line.trim_end(),
                    line,
                    "{t:?} line {} has trailing whitespace: {line:?}",
                    i + 1
                );
            }
        });
    }

    #[test]
    fn ends_with_exactly_one_newline() {
        each_target(|t, out| {
            assert!(out.ends_with('\n'), "{t:?} must end with a newline");
            assert!(
                !out.ends_with("\n\n"),
                "{t:?} must not end with a blank line"
            );
        });
    }

    #[test]
    fn no_tabs() {
        each_target(|t, out| assert!(!out.contains('\t'), "{t:?} contains a tab"));
    }

    /// rustfmt collapses consecutive blank lines, so emitting them means every
    /// `cargo fmt` rewrites the file and breaks the drift guard. This is the
    /// third distinct way that has happened; the check is now structural.
    #[test]
    fn no_run_of_blank_lines() {
        each_target(|t, out| {
            let mut blank = 0;
            for (i, line) in out.lines().enumerate() {
                blank = if line.trim().is_empty() { blank + 1 } else { 0 };
                assert!(blank < 2, "{t:?} has consecutive blank lines at {}", i + 1);
            }
        });
    }

    /// The whole point: what jedem writes is what rustfmt would leave alone.
    #[test]
    fn output_is_already_normalised() {
        each_target(|t, out| {
            assert_eq!(
                super::normalise(out),
                out,
                "{t:?} is not normalisation-stable"
            )
        });
    }
}

#[cfg(test)]
mod node_tests {
    use super::tests_support::SURFACE;
    use super::{generate, Target};

    fn node() -> String {
        generate(&SURFACE, Target::Node, "mycore")
    }

    #[test]
    fn names_are_camel_case_in_js_and_snake_case_in_rust() {
        let out = node();
        // The Rust fn keeps its own name; only the exported spelling changes.
        assert!(out.contains("pub fn greet("), "{out}");
        assert!(out.contains(r#"#[napi(js_name = "greet")]"#));
    }

    #[test]
    fn a_pinned_name_wins_over_camel_casing() {
        assert!(node().contains(r#"#[napi(js_name = "checked_alias")]"#));
    }

    #[test]
    fn napi_takes_owned_strings_and_the_call_re_borrows() {
        let out = node();
        // The core wants `&str`; napi hands over `String`.
        assert!(out.contains("pub fn greet(name: String)"), "{out}");
        assert!(out.contains("mycore::Hello::greet(&name)"), "{out}");
    }

    #[test]
    fn a_synchronous_function_stays_synchronous() {
        let out = node();
        assert!(!out.contains("AsyncTask"), "no promise machinery");
        assert!(!out.contains("Promise"));
    }

    #[test]
    fn a_fallible_op_throws() {
        assert!(node().contains("-> napi::Result<()>"));
    }
}

#[cfg(test)]
mod camel {
    #[test]
    fn cases() {
        assert_eq!(super::lower_camel("complete_json"), "completeJson");
        assert_eq!(super::lower_camel("reverse_bytes"), "reverseBytes");
        assert_eq!(super::lower_camel("greet"), "greet");
        assert_eq!(super::lower_camel("a_b_c"), "aBC");
    }
}

#[cfg(test)]
mod rustfmt_stability {
    use super::*;

    /// Generated Rust is already formatted the way rustfmt would format it.
    ///
    /// Generated crates are workspace members, so `cargo fmt --all` rewrites
    /// them like any other source. Every time the generator's layout differed
    /// from rustfmt's, the next `cargo fmt` silently edited the committed
    /// bindings and the drift guard failed pointing at the surface -- which
    /// nobody had touched. Marking the files `#![rustfmt::skip]` would say this
    /// directly, but custom inner attributes are still unstable, so instead the
    /// generator agrees with rustfmt and this test holds it to that.
    ///
    /// Skipped when rustfmt is not installed, so the suite still runs on a
    /// toolchain without it.
    #[test]
    fn generated_rust_is_already_rustfmt_clean() {
        let Some(formatted_of) = rustfmt() else {
            eprintln!("rustfmt not available; skipping");
            return;
        };
        for &target in Target::ALL {
            for file in generate_crate(&tests_support::SURFACE, target, "demo", "..", "b", "demo") {
                if !file.path.ends_with(".rs") {
                    continue;
                }
                let formatted = formatted_of(&file.contents);
                assert_eq!(
                    file.contents,
                    formatted,
                    "\n\n{}/{} is not what rustfmt would write, so `cargo fmt` will \
                     edit the committed bindings and break the drift guard.\n",
                    target.dir_name(),
                    file.path
                );
            }
        }
    }

    /// A `source -> formatted` function, or `None` if rustfmt is missing.
    fn rustfmt() -> Option<fn(&str) -> String> {
        use std::process::{Command, Stdio};
        Command::new("rustfmt").arg("--version").output().ok()?;
        fn run(src: &str) -> String {
            use std::io::Write;
            let mut c = Command::new("rustfmt")
                .args(["--emit", "stdout", "--edition", "2021", "--quiet"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn rustfmt");
            c.stdin
                .take()
                .unwrap()
                .write_all(src.as_bytes())
                .expect("write to rustfmt");
            let out = c.wait_with_output().expect("rustfmt");
            String::from_utf8(out.stdout).expect("utf-8")
        }
        Some(run)
    }
}
