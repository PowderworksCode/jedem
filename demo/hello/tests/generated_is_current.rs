//! The drift guard, for every backend.
//!
//! jedem serialises nothing, so there is no interchange document that can go
//! stale — but the *generated bindings* are committed, and those can. This
//! regenerates in memory and diffs against what is on disk, so a surface
//! change nobody regenerated fails the build rather than shipping a binding
//! that describes an older API.
//!
//! It is stronger than the staleness check a serialising design needs: it
//! catches generator changes too, not just surface changes. And it must cover
//! *every* backend, or adding a language quietly halves the guarantee.

fn check(target: jedem::Target, committed: &str, path: &str) {
    let fresh = jedem::generate(hello::JEDEM_SURFACE, target, "hello");
    assert_eq!(
        committed, fresh,
        "\n\n{path} is out of date.\nrun: cargo run -p hello --bin generate\n"
    );
}

#[test]
fn the_committed_python_binding_matches_the_surface() {
    check(
        jedem::Target::Python,
        include_str!("../../hello-py/src/generated.rs"),
        "demo/hello-py/src/generated.rs",
    );
}

#[test]
fn the_committed_node_binding_matches_the_surface() {
    check(
        jedem::Target::Node,
        include_str!("../../hello-node/src/generated.rs"),
        "demo/hello-node/src/generated.rs",
    );
}

/// Every target must be covered above. A new backend that nobody added a guard
/// for is a backend whose committed output can rot unnoticed.
#[test]
fn every_target_has_a_drift_guard() {
    // Update this list *and* add a test above when adding a backend.
    let guarded = [jedem::Target::Python, jedem::Target::Node];
    assert_eq!(
        guarded.len(),
        jedem::Target::ALL.len(),
        "jedem::Target::ALL has {} entries but only {} are drift-guarded",
        jedem::Target::ALL.len(),
        guarded.len()
    );
    for t in jedem::Target::ALL {
        assert!(guarded.contains(t), "{t:?} has no drift guard");
    }
}
