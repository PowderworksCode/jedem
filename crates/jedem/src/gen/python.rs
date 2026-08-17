//! The Python backend: a [pyo3](https://pyo3.rs) extension module.
//!
//! What a Python developer would have written by hand. A synchronous Rust
//! function becomes a synchronous Python function, not a coroutine. A `Result`
//! becomes a raised exception, not an error value the caller must remember to
//! inspect. Names stay `snake_case`, because they already are.

use crate::descriptor::{Op, Surface, Type};

pub(super) fn generate(surface: &Surface, core_path: &str) -> String {
    let mut out = String::new();
    out.push_str(&super::banner(surface, "python"));
    out.push_str(
        r#"
#![allow(clippy::all)]
#![allow(unused_imports)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// A core error surfaces as a Python exception carrying its `Display` text --
/// the idiomatic seam, rather than an error value the caller has to remember
/// to inspect.
fn err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}
"#,
    );

    let mut registrations = Vec::new();
    for iface in surface.interfaces {
        out.push_str(&format!("\n// ---- {} ----\n\n", iface.name));
        for op in iface.ops {
            let exported = op.export_name.unwrap_or(op.name);
            out.push_str(&op_fn(op, exported, core_path));
            out.push('\n');
            registrations.push(exported.to_string());
        }
    }

    out.push_str(&format!(
        "/// Register everything on the module. Call this from your `#[pymodule]`.\npub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {{\n{}    Ok(())\n}}\n",
        registrations
            .iter()
            .map(|n| format!("    m.add_function(wrap_pyfunction!({n}, m)?)?;\n"))
            .collect::<String>()
    ));
    out
}

fn op_fn(op: &Op, exported: &str, core_path: &str) -> String {
    let mut s = String::new();
    if let Some(doc) = op.doc {
        for line in doc.lines() {
            // An empty doc line is `///`, not `/// ` -- generated output has to
            // be rustfmt-stable, or `cargo fmt` rewrites it and the drift guard
            // fights the formatter forever.
            if line.is_empty() {
                s.push_str("///\n");
            } else {
                s.push_str(&format!("/// {line}\n"));
            }
        }
    }
    let params: Vec<String> = op
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, param_ty(&p.ty)))
        .collect();
    let args: Vec<String> = op.params.iter().map(|p| p.name.to_string()).collect();

    s.push_str("#[pyfunction]\n");
    if exported != op.name {
        s.push_str(&format!("#[pyo3(name = \"{exported}\")]\n"));
    }
    s.push_str(&format!(
        "pub fn {}({}) -> {} {{\n",
        exported,
        params.join(", "),
        return_ty(op)
    ));

    let call = format!("{}::{}({})", core_path, op.rust_path, args.join(", "));
    s.push_str(&match (op.fallible, op.returns) {
        (true, Type::Unit) => format!("    {call}.map_err(err)?;\n    Ok(())\n"),
        (true, _) => format!("    {call}.map_err(err)\n"),
        (false, Type::Unit) => format!("    {call};\n"),
        (false, _) => format!("    {call}\n"),
    });
    s.push_str("}\n");
    s
}

/// A parameter's Rust spelling in the generated signature.
///
/// Bytes are position-aware: a parameter is a borrowed view (`&[u8]`), so
/// Python's `bytes` crosses without a copy on our side, while a *return* is
/// owned.
fn param_ty(t: &Type) -> String {
    match t {
        Type::Str => "&str".into(),
        Type::Bytes => "&[u8]".into(),
        other => owned_ty(other),
    }
}

/// The owned spelling, used for returns and inside containers.
fn owned_ty(t: &Type) -> String {
    match t {
        Type::Unit => "()".into(),
        Type::Bool => "bool".into(),
        Type::I32 => "i32".into(),
        Type::I64 => "i64".into(),
        Type::F64 => "f64".into(),
        Type::Str => "String".into(),
        Type::Bytes => "Vec<u8>".into(),
        Type::Optional(inner) => format!("Option<{}>", owned_ty(inner)),
        Type::List(inner) => format!("Vec<{}>", owned_ty(inner)),
    }
}

fn return_ty(op: &Op) -> String {
    let inner = owned_ty(&op.returns);
    if op.fallible {
        format!("PyResult<{inner}>")
    } else {
        inner
    }
}
