//! The jedem demo surface.
//!
//! Ordinary Rust with one attribute. `#[jedem::export]` captures the shape; it
//! does not change what these functions do, and they stay callable from Rust
//! exactly as written — which is the test in `tests/rust_still_works.rs`.
//!
//! Note what is *not* here: no FFI, no pyo3, no generated code. This crate is
//! the implementation. The binding lives next door in `hello-py`.

/// Greetings and small arithmetic — a surface small enough to read whole.
pub struct Hello;

/// Returned when a name is not something we are willing to greet.
#[derive(Debug, PartialEq, Eq)]
pub struct EmptyName;

impl std::fmt::Display for EmptyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("name must not be empty")
    }
}

impl std::error::Error for EmptyName {}

#[jedem::export]
impl Hello {
    /// Greet someone by name.
    pub fn greet(name: &str) -> String {
        format!("Hello, {name}!")
    }

    /// Greet someone, refusing an empty name.
    ///
    /// Fallible: the `Result` becomes a raised exception in Python, not an
    /// error value the caller has to remember to check.
    pub fn greet_checked(name: &str) -> Result<String, EmptyName> {
        if name.is_empty() {
            return Err(EmptyName);
        }
        Ok(format!("Hello, {name}!"))
    }

    /// Add two integers. Infallible, so there is no error seam at all.
    pub fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    /// Repeat some text, or nothing at all when `times` is not positive.
    pub fn repeat(text: &str, times: i32) -> Option<String> {
        if times <= 0 {
            None
        } else {
            Some(text.repeat(times as usize))
        }
    }

    /// Split text on a separator.
    pub fn split(text: &str, sep: &str) -> Vec<String> {
        if sep.is_empty() {
            return vec![text.to_string()];
        }
        text.split(sep).map(|s| s.to_string()).collect()
    }

    /// Reverse a byte string: bytes in, bytes out.
    pub fn reverse_bytes(data: &[u8]) -> Vec<u8> {
        data.iter().rev().copied().collect()
    }

    /// Exported under a pinned name, to prove the spelling is respected.
    #[jedem(name = "shout")]
    pub fn shout_it(text: &str) -> String {
        text.to_uppercase()
    }
}

jedem::surface! { name: "hello", version: "0.1.0", api: [Hello] }
