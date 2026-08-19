//! Rust is a first-class jedem target by doing nothing at all.
//!
//! There is no FFI boundary between Rust and Rust, so there is nothing to
//! generate. What "Rust support" means is that the annotations are **inert**:
//! an exported function stays an ordinary associated function, callable
//! exactly as written, and the descriptor rides alongside as pure data.

use hello::{Hello, JEDEM_SURFACE};

#[test]
fn exported_functions_are_ordinary_rust() {
    assert_eq!(Hello::greet("world"), "Hello, world!");
    assert_eq!(Hello::add(2, 40), 42);
    assert_eq!(Hello::greet_checked(""), Err(hello::EmptyName));
    assert_eq!(Hello::greet_checked("x").unwrap(), "Hello, x!");
    assert_eq!(Hello::repeat("ab", 2), Some("abab".into()));
    assert_eq!(Hello::repeat("ab", 0), None);
    assert_eq!(Hello::split("a,b", ","), ["a", "b"]);
    assert_eq!(Hello::reverse_bytes(&[1, 2, 3]), [3, 2, 1]);
    // A pinned export name changes the binding, never the Rust name.
    assert_eq!(Hello::shout_it("hi"), "HI");
}

#[test]
fn the_descriptor_describes_what_was_written() {
    assert_eq!(JEDEM_SURFACE.name, "hello");
    assert_eq!(JEDEM_SURFACE.version, "0.1.0");
    // Two types and two modules, to prove `api:` takes every form -- including
    // a handle (`Counter`) alongside plain namespaces.
    assert_eq!(JEDEM_SURFACE.interfaces.len(), 4);
    let names: Vec<&str> = JEDEM_SURFACE.interfaces.iter().map(|i| i.name).collect();
    assert_eq!(names, ["Hello", "fallible", "ripeness", "Counter"]);

    let iface = JEDEM_SURFACE.interfaces[0];
    assert_eq!(iface.name, "Hello");
    let names: Vec<&str> = iface.ops.iter().map(|o| o.name).collect();
    assert_eq!(
        names,
        [
            "greet",
            "greet_checked",
            "add",
            "repeat",
            "split",
            "reverse_bytes",
            "shout_it"
        ]
    );
}

#[test]
fn fallibility_is_inferred_from_the_signature() {
    let ops = JEDEM_SURFACE.interfaces[0].ops;
    let by = |n: &str| ops.iter().find(|o| o.name == n).unwrap();
    assert!(!by("greet").fallible, "-> String has no error seam");
    assert!(by("greet_checked").fallible, "-> Result<_, _> does");
    assert_eq!(
        by("greet_checked").returns,
        jedem::Type::Str,
        "the Result wrapper is unwrapped; the payload is what crosses"
    );
}

#[test]
fn types_lower_as_expected() {
    use jedem::Type;
    let ops = JEDEM_SURFACE.interfaces[0].ops;
    let by = |n: &str| ops.iter().find(|o| o.name == n).unwrap();
    assert_eq!(by("add").returns, Type::I64);
    assert_eq!(by("repeat").returns, Type::Optional(&Type::Str));
    assert_eq!(by("split").returns, Type::List(&Type::Str));
    // Vec<u8> is bytes, not a list of small integers.
    assert_eq!(by("reverse_bytes").returns, Type::Bytes);
    assert_eq!(by("reverse_bytes").params[0].ty, Type::Bytes);
}

#[test]
fn doc_comments_and_pinned_names_are_captured() {
    let ops = JEDEM_SURFACE.interfaces[0].ops;
    let by = |n: &str| ops.iter().find(|o| o.name == n).unwrap();
    assert!(by("greet").doc.unwrap().contains("Greet someone by name"));
    assert_eq!(by("shout_it").export_name, Some("shout"));
    assert_eq!(
        by("greet").export_name,
        None,
        "unpinned keeps the Rust name"
    );
}

#[test]
fn a_skipped_method_stays_rust_only() {
    let mut c = hello::Counter::new();
    c.add(7);
    // Callable from Rust exactly as written...
    assert_eq!(c.into_total(), 7);

    // ...and absent from the surface, so no backend has to lower it.
    let counter = JEDEM_SURFACE
        .interfaces
        .iter()
        .find(|i| i.name == "Counter")
        .unwrap();
    assert!(counter.handle);
    let names: Vec<&str> = counter.ops.iter().map(|o| o.name).collect();
    assert!(!names.contains(&"into_total"), "got {names:?}");
    assert_eq!(
        names,
        [
            "new",
            "starting_at",
            "with_total",
            "add",
            "total",
            "steps",
            "halve"
        ]
    );
}

#[test]
fn a_builder_is_classified_as_one_and_stays_a_builder_in_rust() {
    // Unannotated, unchanged, and still the ordinary Rust move-builder.
    let c = hello::Counter::new().with_total(10);
    assert_eq!(c.total(), 10);

    let counter = JEDEM_SURFACE
        .interfaces
        .iter()
        .find(|i| i.name == "Counter")
        .unwrap();
    let op = counter
        .ops
        .iter()
        .find(|o| o.name == "with_total")
        .expect("with_total is exported without any annotation on it");
    assert_eq!(op.kind, jedem::OpKind::Builder);
    assert!(
        counter.consuming,
        "a handle with a builder has to move its value out, so it stores an Option"
    );
}
