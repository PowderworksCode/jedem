//! The Python binding for `hello`.
//!
//! Everything of substance is in `generated.rs`, which is committed so a
//! reviewer can read exactly what crosses the boundary. This file is the four
//! lines that cannot be generated: naming the module.

mod generated;

#[pyo3::pymodule]
fn hello(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    generated::register(m)
}
