//! The whole of jedem's build step, for this crate.
//!
//!     cargo jedem generate

jedem::generator_main! {
    surface: hello::JEDEM_SURFACE,
    core: "hello",
    out: "..",
}
