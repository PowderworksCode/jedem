//! Regenerate the bindings.
//!
//! This is the whole of jedem's build step: a bin target that links the
//! generator as a library and calls it on the descriptors the macros produced.
//! Nothing is serialised in between — there is no schema, no interchange file,
//! and nothing that can go stale against the code it describes.
//!
//!     cargo run -p hello --bin generate

fn main() -> std::io::Result<()> {
    for (target, out) in [
        (jedem::Target::Python, "hello-py"),
        (jedem::Target::Node, "hello-node"),
    ] {
        let path = format!("{}/../{}/src/generated.rs", env!("CARGO_MANIFEST_DIR"), out);
        let code = jedem::generate(hello::JEDEM_SURFACE, target, "hello");
        std::fs::write(&path, &code)?;
        println!("wrote {} ({} bytes)", path, code.len());
    }
    Ok(())
}
