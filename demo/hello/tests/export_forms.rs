//! `#[jedem::export]` accepts three forms, and they capture the same things.
//!
//! The bare `fn` and `mod` forms exist so a crate exporting free functions does
//! not have to invent a type for them to hang off. `pub struct Api;` conveys
//! nothing, and every consumer was writing one.

/// A single exported function, with no surrounding type.
#[jedem::export]
pub fn shout(text: &str) -> String {
    text.to_uppercase()
}

/// A module's worth of exported functions.
#[jedem::export]
pub mod arithmetic {
    /// Add two numbers.
    pub fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    /// Halve a number, refusing odd ones.
    pub fn halve(a: i64) -> Result<i64, String> {
        if a % 2 != 0 {
            return Err(format!("{a} is odd"));
        }
        Ok(a / 2)
    }

    /// Private functions are not exported.
    #[allow(dead_code)]
    fn helper() -> i64 {
        0
    }
}

#[test]
fn a_bare_function_stays_callable() {
    // The annotation is inert: this is an ordinary function.
    assert_eq!(shout("hi"), "HI");
}

#[test]
fn a_module_stays_callable() {
    assert_eq!(arithmetic::add(2, 40), 42);
    assert_eq!(arithmetic::halve(9), Err("9 is odd".into()));
}

#[test]
fn every_form_exposes_the_interface_at_the_same_path() {
    // A type, a module and a bare function are all reached identically, which
    // is what lets `surface! { api: [...] }` read uniformly.
    assert_eq!(hello::Hello::JEDEM_INTERFACE.name, "Hello");
    assert_eq!(arithmetic::JEDEM_INTERFACE.name, "arithmetic");
    assert_eq!(shout::JEDEM_INTERFACE.name, "shout");
}

#[test]
fn a_bare_function_captures_what_an_impl_would() {
    let iface = shout::JEDEM_INTERFACE;
    assert_eq!(iface.ops.len(), 1);
    let op = &iface.ops[0];
    assert_eq!(op.name, "shout");
    assert_eq!(op.rust_path, "shout", "no type prefix to call through");
    assert_eq!(op.params[0].ty, jedem::Type::Str);
    assert!(op.params[0].borrowed);
    assert!(!op.fallible);
    assert!(op.doc.unwrap().contains("single exported function"));
}

#[test]
fn a_module_exports_only_its_public_functions() {
    let iface = arithmetic::JEDEM_INTERFACE;
    let names: Vec<&str> = iface.ops.iter().map(|o| o.name).collect();
    assert_eq!(names, ["add", "halve"], "`helper` is private");

    let halve = iface.ops.iter().find(|o| o.name == "halve").unwrap();
    assert!(halve.fallible, "-> Result<_, _>");
    assert_eq!(halve.returns, jedem::Type::I64, "the Result is unwrapped");
    assert_eq!(
        halve.rust_path, "arithmetic::halve",
        "calls go through the module"
    );
}

#[test]
fn all_three_forms_generate() {
    // The point of the uniform path: one surface, three kinds of entry.
    const SURFACE: jedem::Surface = jedem::Surface {
        name: "mixed",
        version: "0.0.0",
        interfaces: &[
            hello::Hello::JEDEM_INTERFACE,
            arithmetic::JEDEM_INTERFACE,
            shout::JEDEM_INTERFACE,
        ],
    };
    let py = jedem::generate(&SURFACE, jedem::Target::Python, "core");
    assert!(py.contains("pub fn shout(text: &str) -> String"), "{py}");
    assert!(
        py.contains("core::shout(text)"),
        "a bare fn needs no prefix"
    );
    assert!(py.contains("core::arithmetic::add(a, b)"), "a mod does");
    assert!(py.contains("core::Hello::greet(name)"), "a type does");
}

/// Any error type works, because jedem never inspects one.
///
/// Every backend renders failure as that language's own mechanism -- a raised
/// exception, a thrown `Error` -- carrying the error's `Display` text. So a
/// function that can fail two ways needs no unifying error enum and no
/// `.map_err(|e| e.to_string())`; `Box<dyn Error>` is enough.
#[test]
fn any_display_error_lowers_as_fallible() {
    let ops = hello::fallible::JEDEM_INTERFACE.ops;
    let by = |n: &str| ops.iter().find(|o| o.name == n).unwrap();

    // Box<dyn Error> -- two failure types behind one signature.
    let boxed = by("halve_parsed");
    assert!(boxed.fallible);
    assert_eq!(boxed.returns, jedem::Type::I64, "the Result is unwrapped");

    // A concrete error type is unchanged.
    let concrete = by("checked");
    assert!(concrete.fallible);
    assert_eq!(concrete.returns, jedem::Type::Str);
}

#[test]
fn a_boxed_error_generates_the_same_seam_as_a_concrete_one() {
    const SURFACE: jedem::Surface = jedem::Surface {
        name: "e",
        version: "0.0.0",
        interfaces: &[hello::fallible::JEDEM_INTERFACE],
    };
    for (target, seam) in [
        (jedem::Target::Python, "PyResult<i64>"),
        (jedem::Target::Node, "napi::Result<i64>"),
    ] {
        let out = jedem::generate(&SURFACE, target, "core");
        assert!(out.contains(seam), "{target:?} should raise: {out}");
        assert!(out.contains("map_err(err)"), "{target:?}");
    }
}

/// Enums cross as each language's own enum, not as a string.
#[test]
fn an_enum_lowers_with_its_variants() {
    let def = <hello::Ripeness as jedem::EnumType>::DEF;
    assert_eq!(def.name, "Ripeness");
    let names: Vec<&str> = def.variants.iter().map(|v| v.name).collect();
    assert_eq!(names, ["Missing", "Partial", "Done", "NotApplicable"]);
    // A pinned boundary spelling differs from the Rust name; the rest match.
    let pinned = def
        .variants
        .iter()
        .find(|v| v.name == "NotApplicable")
        .unwrap();
    assert_eq!(pinned.wire, "not_applicable");
    assert_eq!(def.variants[0].wire, "Missing");
    assert!(def.variants[2].doc.unwrap().contains("will not change"));
}

#[test]
fn an_enum_is_found_through_containers() {
    // `Option<Ripeness>` must still declare the enum, or the binding names a
    // type it never defined.
    let ops = hello::ripeness::JEDEM_INTERFACE.ops;
    let maybe = ops.iter().find(|o| o.name == "maybe").unwrap();
    assert_eq!(maybe.returns.enum_def().map(|d| d.name), Some("Ripeness"));
}

#[test]
fn each_language_declares_the_enum_its_own_way() {
    const SURFACE: jedem::Surface = jedem::Surface {
        name: "e",
        version: "0.0.0",
        interfaces: &[hello::ripeness::JEDEM_INTERFACE],
    };
    let py = jedem::generate(&SURFACE, jedem::Target::Python, "core");
    assert!(py.contains("#[pyclass(eq, eq_int)]"), "a real Python class");
    assert!(py.contains(r#"#[pyo3(name = "not_applicable")]"#));
    assert!(
        py.contains("m.add_class::<Ripeness>()"),
        "registered on the module"
    );
    // Conversion is mapped through the container, since From<A> for B does not
    // give Option<A> -> Option<B>.
    assert!(py.contains(".map(|v| v.into())"), "{py}");

    let node = jedem::generate(&SURFACE, jedem::Target::Node, "core");
    assert!(node.contains("#[napi(string_enum)]"), "a TS string union");
    assert!(
        node.contains("core::ripeness::is_settled(r.into())"),
        "{node}"
    );
}

/// Handles: an interface with a constructor and methods.
#[test]
fn a_handle_is_classified_as_one() {
    use jedem::OpKind;
    let iface = hello::Counter::JEDEM_INTERFACE;
    assert!(iface.handle, "Counter has a ctor and methods");
    let by = |n: &str| iface.ops.iter().find(|o| o.name == n).unwrap();

    assert_eq!(by("new").kind, OpKind::Ctor);
    assert_eq!(
        by("starting_at").kind,
        OpKind::Ctor,
        "-> Result<Self, _> too"
    );
    assert_eq!(by("add").kind, OpKind::Method { mutable: true });
    assert_eq!(by("total").kind, OpKind::Method { mutable: false });
    assert!(by("halve").fallible);
}

#[test]
fn a_namespace_is_not_a_handle() {
    assert!(!hello::ripeness::JEDEM_INTERFACE.handle);
    assert!(
        !hello::Hello::JEDEM_INTERFACE.handle,
        "no receivers, no ctor"
    );
}

#[test]
fn each_language_gets_one_constructor_and_factories_for_the_rest() {
    const SURFACE: jedem::Surface = jedem::Surface {
        name: "h",
        version: "0.0.0",
        interfaces: &[hello::Counter::JEDEM_INTERFACE],
    };
    let py = jedem::generate(&SURFACE, jedem::Target::Python, "core");
    assert!(py.contains("#[pyclass]"), "a real class");
    assert!(
        py.contains("inner: core::Counter"),
        "owns the core value directly"
    );
    assert_eq!(py.matches("#[new]").count(), 1, "Python has one __new__");
    assert!(py.contains("#[staticmethod]\n    fn starting_at"), "{py}");
    assert!(
        py.contains("fn add(&mut self, n: i64) {"),
        "no `-> ()` noise: {py}"
    );

    let node = jedem::generate(&SURFACE, jedem::Target::Node, "core");
    assert_eq!(node.matches("#[napi(constructor)]").count(), 1);
    assert!(
        node.contains("#[napi(factory, js_name = \"startingAt\")]"),
        "{node}"
    );
    assert!(node.contains("pub fn add(&mut self, n: i64) {"), "{node}");
}
