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

/// Regenerate in memory and diff against every committed file, for every
/// target. Now covers the whole crate -- manifest, shims and build script --
/// not just the binding source, since jedem writes all of them.
fn check(target: jedem::Target, dir: &str) {
    for file in jedem::generate_crate(
        hello::JEDEM_SURFACE,
        target,
        "hello",
        "../hello",
        &format!("hello-{}", target.dir_name()),
    ) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(dir)
            .join(&file.path);
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is missing: {e}", path.display()));
        assert_eq!(
            committed, file.contents,
            "\n\ndemo/{dir}/{} is out of date.\nrun: cargo jedem generate\n",
            file.path
        );
    }
}

#[test]
fn the_committed_python_crate_matches_the_surface() {
    check(jedem::Target::Python, "python");
}

#[test]
fn the_committed_node_crate_matches_the_surface() {
    check(jedem::Target::Node, "node");
}

/// Every target must be covered above. A new backend nobody added a guard for
/// is a backend whose committed output can rot unnoticed.
#[test]
fn every_target_has_a_drift_guard() {
    let guarded = [jedem::Target::Python, jedem::Target::Node];
    assert_eq!(guarded.len(), jedem::Target::ALL.len());
    for t in jedem::Target::ALL {
        assert!(guarded.contains(t), "{t:?} has no drift guard");
    }
}

/// Every file jedem writes carries the `@generated` marker review tools look
/// for, so nobody has to guess which files are hand-written.
#[test]
fn every_generated_file_is_marked_as_generated() {
    for &target in jedem::Target::ALL {
        for file in
            jedem::generate_crate(hello::JEDEM_SURFACE, target, "hello", "../hello", "hello-x")
        {
            let head: String = file.contents.lines().take(3).collect::<Vec<_>>().join("\n");
            assert!(
                head.contains("@generated"),
                "{:?}/{} has no @generated marker; it starts:\n{head}",
                target,
                file.path
            );
        }
    }
}
