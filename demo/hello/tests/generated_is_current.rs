//! The drift guard.
//!
//! jedem serialises nothing, so there is no interchange document that can go
//! stale — but the *generated binding* is committed, and that can. This test
//! regenerates in memory and diffs against what is on disk, so a surface
//! change that nobody regenerated fails the build rather than shipping a
//! binding that describes an older API.
//!
//! It replaces the "is the checked-in catalog current?" check a serialising
//! design needs, and it is a stronger gate: it catches generator changes too,
//! not just front-end ones.

#[test]
fn the_committed_binding_matches_the_surface() {
    let committed = include_str!("../../hello-py/src/generated.rs");
    let fresh = jedem::generate(hello::JEDEM_SURFACE, jedem::Target::Python, "hello");
    assert_eq!(
        committed, fresh,
        "\n\nthe committed binding is out of date.\n\
         run: cargo run -p hello --bin generate\n"
    );
}
