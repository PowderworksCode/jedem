//! The Node binding for `hello`.
//!
//! Everything of substance is in `generated.rs`, which is committed so a
//! reviewer can read exactly what crosses the boundary. napi registers each
//! `#[napi]` function at module load, so unlike pyo3 there is not even a
//! module function to write here.

mod generated;
