//! Regenerate the bindings.
//!
//! This is the whole of jedem's build step: a bin target that links the
//! generator as a library and calls it on the descriptors the macros produced.
//! Nothing is serialised in between — there is no schema, no interchange file,
//! and nothing that can go stale against the code it describes.
//!
//!     cargo run -p hello --bin generate

fn main() -> std::io::Result<()> {
    let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../hello-py/src/generated.rs");
    let code = jedem::generate(hello::JEDEM_SURFACE, jedem::Target::Python, "hello");
    std::fs::write(out, &code)?;
    println!("wrote {} ({} bytes)", out, code.len());
    Ok(())
}
