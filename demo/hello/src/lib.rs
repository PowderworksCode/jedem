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
    /// Fallible: the `Result` becomes whatever that language uses for failure
    /// -- a raised exception, a thrown error -- not an error value the caller
    /// has to remember to check. A doc comment is written once and read in
    /// every binding, so it should not name one language.
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

/// How far along a value is — the shape jawohl's `Syntax` has, and the reason
/// enums exist: without them this crossed as a magic string.
#[derive(jedem::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ripeness {
    /// Not there at all.
    Missing,
    /// Present but still arriving.
    Partial,
    /// Finished; it will not change.
    Done,
    /// A pinned boundary spelling, for a value whose name is already fixed.
    #[jedem(name = "not_applicable")]
    NotApplicable,
}

/// Functions over an enum, in both directions.
#[jedem::export]
pub mod ripeness {
    use super::Ripeness;

    /// Classify a length. Returns an enum.
    pub fn classify(len: i32) -> Ripeness {
        match len {
            i32::MIN..=-1 => Ripeness::NotApplicable,
            0 => Ripeness::Missing,
            1..=9 => Ripeness::Partial,
            _ => Ripeness::Done,
        }
    }

    /// Is it safe to act on? Takes an enum.
    pub fn is_settled(r: Ripeness) -> bool {
        matches!(r, Ripeness::Done)
    }

    /// An enum inside an Option.
    pub fn maybe(present: bool) -> Option<Ripeness> {
        present.then_some(Ripeness::Done)
    }
}

jedem::surface! { name: "hello", version: "0.1.0", api: [Hello, fallible, ripeness] }

/// Errors that can come from more than one place.
///
/// Before `Box<dyn Error>` was accepted, a function that could fail two ways
/// had to flatten to `Result<_, String>` and litter itself with
/// `.map_err(|e| e.to_string())`. jedem never inspected the error type — every
/// backend renders failure as that language's own mechanism carrying the
/// error's `Display` text — so anything `Display` works.
#[jedem::export]
pub mod fallible {
    use std::error::Error;

    /// Parse a number, then halve it. Two different failure types, one
    /// signature, no `map_err` in sight.
    pub fn halve_parsed(text: &str) -> Result<i64, Box<dyn Error>> {
        let n: i64 = text.parse()?;
        if n % 2 != 0 {
            return Err(format!("{n} is odd").into());
        }
        Ok(n / 2)
    }

    /// A plain concrete error still works, unchanged.
    pub fn checked(text: &str) -> Result<String, super::EmptyName> {
        if text.is_empty() {
            return Err(super::EmptyName);
        }
        Ok(text.to_string())
    }
}
