//! The macros. Expand to **pure data** — never to behaviour, and never to a
//! file written at expansion time.
//!
//! Use them through the `jedem` crate; this one is an implementation detail.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, FnArg, ImplItem, Item, ItemFn, ItemImpl, ItemMod, Lit,
    Meta, ReturnType, Type,
};

/// Mark functions for export.
///
/// Accepts an `impl` block, a `mod`, or a single `fn`:
///
/// ```ignore
/// #[jedem::export]
/// pub fn greet(name: &str) -> String { … }
///
/// #[jedem::export]
/// mod api {
///     pub fn greet(name: &str) -> String { … }
/// }
///
/// #[jedem::export]
/// impl Greeter { pub fn greet(name: &str) -> String { … } }
/// ```
///
/// Whatever the form, what you wrote is emitted unchanged — the functions you
/// wrote are the functions that run — alongside a constant describing them.
/// Drift between declaration and implementation is impossible because there is
/// only one artefact.
///
/// A bare `fn` or a `mod` exists so that a crate exporting free functions does
/// not have to invent a type for them to hang off. `pub struct Api;` conveys
/// nothing, and every consumer was writing one.
#[proc_macro_attribute]
pub fn export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(item as Item);
    let expanded = match &parsed {
        Item::Impl(i) => expand_export(i),
        Item::Fn(f) => expand_fn(f),
        Item::Mod(m) => expand_mod(m),
        other => Err(syn::Error::new(
            other.span(),
            "#[jedem::export] goes on an `impl` block, a `mod`, or a `fn`",
        )),
    };
    match expanded {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// A single exported function. The interface is named after the function, and
/// the surface lists it directly.
fn expand_fn(f: &ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let op = lower_fn(&f.sig, &f.attrs, &f.vis, "")?.ok_or_else(|| {
        syn::Error::new(
            f.sig.ident.span(),
            "#[jedem::export] on a function needs it to be `pub`",
        )
    })?;
    let name = f.sig.ident.to_string();
    let ident = &f.sig.ident;
    let doc = opt_str(doc_of(&f.attrs).as_deref());
    let cleaned = strip_jedem_attrs_fn(f);
    // Rust keeps modules and functions in separate namespaces, so a module
    // named after the function can carry its descriptor without shadowing it.
    // That is what lets `surface! { api: [greet] }` read the same for a bare
    // function as for a type.
    Ok(quote! {
        #cleaned

        #[doc(hidden)]
        pub mod #ident {
            /// The jedem descriptor for the function of the same name.
            pub const JEDEM_INTERFACE: &'static ::jedem::Interface = &::jedem::Interface {
                name: #name,
                doc: #doc,
                ops: &[#op],
                handle: false,
            };
        }
    })
}

/// Every public function in a module, as one interface named after the module.
fn expand_mod(m: &ItemMod) -> syn::Result<proc_macro2::TokenStream> {
    let Some((_, items)) = &m.content else {
        return Err(syn::Error::new(
            m.span(),
            "#[jedem::export] needs the module's body, not a `mod foo;` declaration",
        ));
    };
    let mod_name = m.ident.to_string();
    let mut ops = Vec::new();
    for item in items {
        if let Item::Fn(f) = item {
            if let Some(op) = lower_fn(&f.sig, &f.attrs, &f.vis, &format!("{mod_name}::"))? {
                ops.push(op);
            }
        }
    }
    if ops.is_empty() {
        return Err(syn::Error::new(
            m.span(),
            "#[jedem::export] found no public functions in this module",
        ));
    }
    let doc = opt_str(doc_of(&m.attrs).as_deref());
    let mut cleaned = strip_jedem_attrs_mod(m);
    // The descriptor goes inside the module, so it is reached the same way a
    // type's is: `mymod::JEDEM_INTERFACE`.
    let holder: syn::Item = syn::parse_quote! {
        /// The jedem descriptor for this module.
        #[doc(hidden)]
        pub const JEDEM_INTERFACE: &'static ::jedem::Interface = &::jedem::Interface {
            name: #mod_name,
            doc: #doc,
            ops: &[#(#ops),*],
            handle: false,
        };
    };
    if let Some((_, items)) = &mut cleaned.content {
        items.push(holder);
    }
    Ok(quote! { #cleaned })
}

fn strip_jedem_attrs_fn(f: &ItemFn) -> ItemFn {
    let mut out = f.clone();
    out.attrs.retain(|a| !a.path().is_ident("jedem"));
    out
}

fn strip_jedem_attrs_mod(m: &ItemMod) -> ItemMod {
    let mut out = m.clone();
    if let Some((_, items)) = &mut out.content {
        for item in items {
            if let Item::Fn(f) = item {
                f.attrs.retain(|a| !a.path().is_ident("jedem"));
            }
        }
    }
    out
}

fn expand_export(input: &ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    // An attribute macro receives the item with its own helper attributes
    // still attached, and must strip them before re-emitting -- unlike a
    // derive, it cannot declare them. `#[jedem(...)]` is ours; rustc would
    // reject it if we handed it back.
    let mut cleaned = input.clone();
    for item in &mut cleaned.items {
        if let ImplItem::Fn(f) = item {
            f.attrs.retain(|a| !a.path().is_ident("jedem"));
        }
    }

    let self_ty = &input.self_ty;
    let type_name = match &**self_ty {
        Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .ok_or_else(|| syn::Error::new(self_ty.span(), "expected a named type"))?,
        _ => {
            return Err(syn::Error::new(
                self_ty.span(),
                "#[jedem::export] needs an impl on a named type",
            ))
        }
    };

    let mut ops = Vec::new();
    for item in &input.items {
        let ImplItem::Fn(f) = item else { continue };
        if let Some(FnArg::Receiver(r)) = f.sig.inputs.first() {
            if r.reference.is_none() && !has_skip(&f.attrs)? {
                return Err(syn::Error::new(
                    r.span(),
                    "a method taking `self` by value consumes the handle, which has no \
                     meaning once it is owned by another language. Take `&self` or \
                     `&mut self`, make it an associated function, or leave it out with \
                     #[jedem(skip)].",
                ));
            }
        }
        if let Some(op) = lower_fn(&f.sig, &f.attrs, &f.vis, &format!("{type_name}::"))? {
            ops.push(op);
        }
    }

    if ops.is_empty() {
        return Err(syn::Error::new(
            input.span(),
            "#[jedem::export] found no public functions to export",
        ));
    }

    // A handle if anything in the block needs an instance: a method with a
    // receiver, or a constructor that makes one.
    let is_handle = input.items.iter().any(|item| {
        let ImplItem::Fn(f) = item else { return false };
        if !matches!(f.vis, syn::Visibility::Public(_)) {
            return false;
        }
        if has_skip(&f.attrs).unwrap_or(false) {
            return false;
        }
        matches!(f.sig.inputs.first(), Some(FnArg::Receiver(_)))
            || matches!(&f.sig.output, ReturnType::Type(_, t)
                if matches!(unwrap_result(t).unwrap_or(t), Type::Path(p) if p.path.is_ident("Self")))
    });

    let const_name = format_ident!("JEDEM_INTERFACE");
    let iface_doc = opt_str(doc_of(&input.attrs).as_deref());
    let type_name_lit = type_name.as_str();

    Ok(quote! {
        #cleaned

        impl #self_ty {
            /// The jedem descriptor for this impl block. Generated; pure data.
            #[doc(hidden)]
            pub const #const_name: &'static ::jedem::Interface = &::jedem::Interface {
                name: #type_name_lit,
                doc: #iface_doc,
                ops: &[#(#ops),*],
                handle: #is_handle,
            };
        }
    })
}

/// Declare the surface: which exported impls belong to it, and what the
/// module is called in the target language.
///
/// ```ignore
/// jedem::surface! { name: "hello", version: "0.1.0", api: [Greeter] }
/// ```
///
/// Roots are explicit rather than collected by link-section magic
/// (`inventory`, `linkme`), which is flaky on wasm and makes "what is in this
/// surface?" unanswerable by reading the source.
#[proc_macro]
pub fn surface(input: TokenStream) -> TokenStream {
    let decl = parse_macro_input!(input as SurfaceDecl);
    let name = decl.name;
    let version = decl.version;
    let bindings = decl.bindings.clone();
    // Every form `#[jedem::export]` accepts exposes the interface at the same
    // place -- `Path::JEDEM_INTERFACE` -- so `api:` reads uniformly whether the
    // entry names a type, a module, or a bare function.
    let refs = decl.api.iter().map(|t| quote! { #t::JEDEM_INTERFACE });
    // With `bindings:` given, the surface also owns generation: a test keeps
    // the committed bindings honest, and writes them when asked. That removes
    // the last hand-written file a consumer needed -- there is no generator bin
    // to add, and nothing to remember to run.
    let generation = match bindings {
        None => quote!(),
        Some(dir) => quote! {
            #[cfg(test)]
            mod __jedem_bindings {
                /// The bindings must match this surface.
                ///
                /// Regenerate with `JEDEM_WRITE=1 cargo test`, or
                /// `cargo jedem generate`, which does the same thing.
                #[test]
                fn are_current() {
                    ::jedem::__verify_or_write(
                        super::JEDEM_SURFACE,
                        env!("CARGO_PKG_NAME"),
                        #dir,
                        env!("CARGO_MANIFEST_DIR"),
                    );
                }
            }
        },
    };

    quote! {
        /// The jedem surface for this crate.
        pub const JEDEM_SURFACE: &'static ::jedem::Surface = &::jedem::Surface {
            name: #name,
            version: #version,
            interfaces: &[#(#refs),*],
        };

        #generation
    }
    .into()
}

struct SurfaceDecl {
    name: String,
    version: String,
    api: Vec<syn::Path>,
    /// Where to write the binding crates, relative to the manifest. When
    /// given, the surface owns generation and no generator file is needed.
    bindings: Option<String>,
}

impl syn::parse::Parse for SurfaceDecl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut version = None;
        let mut bindings = None;
        let mut api = Vec::new();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![:]>()?;
            match key.to_string().as_str() {
                "name" => name = Some(input.parse::<syn::LitStr>()?.value()),
                "version" => version = Some(input.parse::<syn::LitStr>()?.value()),
                "bindings" => bindings = Some(input.parse::<syn::LitStr>()?.value()),
                "api" => {
                    let content;
                    syn::bracketed!(content in input);
                    let list = content.parse_terminated(syn::Path::parse, syn::Token![,])?;
                    api = list.into_iter().collect();
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key `{other}`; expected name, version, api or bindings"),
                    ))
                }
            }
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(SurfaceDecl {
            name: name
                .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing `name`"))?,
            version: version.unwrap_or_else(|| "0.0.0".into()),
            api,
            bindings,
        })
    }
}

/// Turn one function signature into an `Op`, or `None` when it is not public.
///
/// Shared by all three forms `#[jedem::export]` accepts, so an `impl` block, a
/// `mod` and a bare `fn` cannot drift apart in what they capture.
fn lower_fn(
    sig: &syn::Signature,
    attrs: &[syn::Attribute],
    vis: &syn::Visibility,
    path_prefix: &str,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    if !matches!(vis, syn::Visibility::Public(_)) {
        return Ok(None);
    }
    // `#[jedem(skip)]` leaves a method out of the surface.
    //
    // An exported impl block is usually most of a type's API, and the parts
    // jedem cannot lower yet -- or that make no sense across a boundary -- are
    // the exception. Marking those is less disruptive than splitting the impl
    // in two, and it keeps the annotation on the original method.
    if has_skip(attrs)? {
        return Ok(None);
    }
    if sig.asyncness.is_some() {
        return Err(syn::Error::new(
            sig.asyncness.span(),
            "jedem v1 is synchronous; async is not lowered yet",
        ));
    }

    let name = sig.ident.to_string();
    let doc = doc_of(attrs);
    let export_name = export_name_of(attrs)?;

    // A receiver makes it a method; a `Self` return with no receiver makes it a
    // constructor. Everything else is a plain function.
    let receiver = sig.inputs.iter().find_map(|a| match a {
        FnArg::Receiver(r) => Some(r),
        _ => None,
    });
    let returns_self = match &sig.output {
        ReturnType::Type(_, t) => {
            let inner = unwrap_result(t).unwrap_or(t);
            matches!(inner, Type::Path(p) if p.path.is_ident("Self"))
        }
        ReturnType::Default => false,
    };
    let kind = match (receiver, returns_self) {
        (Some(r), _) => {
            let mutable = r.mutability.is_some();
            quote!(::jedem::OpKind::Method { mutable: #mutable })
        }
        (None, true) => quote!(::jedem::OpKind::Ctor),
        (None, false) => quote!(::jedem::OpKind::Function),
    };

    let mut params = Vec::new();
    for arg in &sig.inputs {
        let FnArg::Typed(pt) = arg else { continue };
        let pname = match &*pt.pat {
            syn::Pat::Ident(i) => i.ident.to_string(),
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "jedem needs a plain parameter name",
                ))
            }
        };
        let ty = lower_type(&pt.ty)?;
        let borrowed = matches!(&*pt.ty, Type::Reference(_));
        params.push((pname, ty, borrowed));
    }

    let (returns, fallible) = match &sig.output {
        ReturnType::Default => (quote!(::jedem::Type::Unit), false),
        ReturnType::Type(_, t) => {
            let inner = unwrap_result(t);
            let fallible = inner.is_some();
            let target = inner.unwrap_or(t);
            if matches!(target, Type::Path(p) if p.path.is_ident("Self")) {
                // A constructor returns the handle; there is no crossing type.
                (quote!(::jedem::Type::Unit), fallible)
            } else {
                (lower_type(target)?, fallible)
            }
        }
    };

    let rust_path = format!("{path_prefix}{name}");
    let doc_tok = opt_str(doc.as_deref());
    let export_tok = opt_str(export_name.as_deref());
    let param_toks = params.iter().map(|(n, t, b)| {
        quote! { ::jedem::Param { name: #n, ty: #t, borrowed: #b } }
    });

    Ok(Some(quote! {
        ::jedem::Op {
            kind: #kind,
            name: #name,
            doc: #doc_tok,
            export_name: #export_tok,
            params: &[#(#param_toks),*],
            returns: #returns,
            fallible: #fallible,
            rust_path: #rust_path,
        }
    }))
}

// ---- helpers ---------------------------------------------------------------

fn opt_str(s: Option<&str>) -> proc_macro2::TokenStream {
    match s {
        Some(v) => quote! { Some(#v) },
        None => quote! { None },
    }
}

fn doc_of(attrs: &[syn::Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|a| match &a.meta {
            Meta::NameValue(nv) if nv.path.is_ident("doc") => match &nv.value {
                syn::Expr::Lit(l) => match &l.lit {
                    Lit::Str(s) => Some(s.value().trim().to_string()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// `#[jedem(name = "...")]` — pin the exported spelling.
fn export_name_of(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for a in attrs {
        if !a.path().is_ident("jedem") {
            continue;
        }
        let mut found = None;
        a.parse_nested_meta(|m| {
            if m.path.is_ident("name") {
                let v: syn::LitStr = m.value()?.parse()?;
                found = Some(v.value());
                Ok(())
            } else {
                Err(m.error("expected `name = \"...\"`"))
            }
        })?;
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

/// `Result<T, E>` -> `T`. Matched by name because a proc macro sees tokens,
/// not resolved types; an aliased `Result` is the known cost of that.
///
/// The error type is deliberately not inspected. Every backend renders failure
/// as that language's own mechanism -- a raised exception, a thrown `Error` --
/// carrying the error's `Display` text, so **anything that implements `Display`
/// works**, including `Box<dyn Error>` and `anyhow::Error`. A `Result` with a
/// single elided parameter (`Result<T>`, from a crate's own alias) is accepted
/// too.
fn unwrap_result(t: &Type) -> Option<&Type> {
    let Type::Path(p) = t else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        // `Result` with no parameters is not a result we can lower.
        return None;
    };
    args.args.first().and_then(|a| match a {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

fn lower_type(t: &Type) -> syn::Result<proc_macro2::TokenStream> {
    match t {
        Type::Reference(r) => lower_type(&r.elem),
        Type::Tuple(t) if t.elems.is_empty() => Ok(quote!(::jedem::Type::Unit)),
        Type::Slice(s) => {
            let inner = lower_type(&s.elem)?;
            if inner.to_string().contains("Bytes") {
                Ok(quote!(::jedem::Type::Bytes))
            } else {
                Ok(quote!(::jedem::Type::List(&#inner)))
            }
        }
        Type::Path(p) => {
            let seg = p
                .path
                .segments
                .last()
                .ok_or_else(|| syn::Error::new(t.span(), "expected a named type"))?;
            let name = seg.ident.to_string();
            match name.as_str() {
                "bool" => Ok(quote!(::jedem::Type::Bool)),
                "i32" | "u32" | "i16" | "u16" | "i8" => Ok(quote!(::jedem::Type::I32)),
                "i64" | "u64" | "isize" | "usize" => Ok(quote!(::jedem::Type::I64)),
                "f64" | "f32" => Ok(quote!(::jedem::Type::F64)),
                "String" | "str" => Ok(quote!(::jedem::Type::Str)),
                "u8" => Ok(quote!(::jedem::Type::Bytes)),
                "Option" => {
                    let inner = generic_arg(seg, t)?;
                    let inner = lower_type(inner)?;
                    Ok(quote!(::jedem::Type::Optional(&#inner)))
                }
                "Vec" => {
                    let inner = generic_arg(seg, t)?;
                    // Vec<u8> is bytes, not a list of small integers.
                    if is_u8(inner) {
                        return Ok(quote!(::jedem::Type::Bytes));
                    }
                    let inner = lower_type(inner)?;
                    Ok(quote!(::jedem::Type::List(&#inner)))
                }
                // Any other named type is taken to be an enum. If it is not,
                // the `EnumType` bound fails with the diagnostic attached to
                // that trait, which says what v1 handles and that there is no
                // opaque-blob fallback.
                _ => Ok(quote!(::jedem::Type::Enum(
                    <#p as ::jedem::EnumType>::DEF
                ))),
            }
        }
        other => Err(syn::Error::new(
            other.span(),
            "jedem cannot lower this type; v1 handles plain values only",
        )),
    }
}

fn is_u8(t: &Type) -> bool {
    matches!(t, Type::Path(p) if p.path.is_ident("u8"))
}

fn generic_arg<'a>(seg: &'a syn::PathSegment, at: &Type) -> syn::Result<&'a Type> {
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return Err(syn::Error::new(at.span(), "expected a generic argument"));
    };
    args.args
        .first()
        .and_then(|a| match a {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        })
        .ok_or_else(|| syn::Error::new(at.span(), "expected a type argument"))
}

/// Derive on a C-like enum so it can cross a language boundary.
///
/// Only unit variants: an enum carrying data is a union, which is a separate
/// feature. Each variant's boundary spelling defaults to its Rust name and can
/// be pinned with `#[jedem(name = "...")]` when a wire value is already fixed.
///
/// ```ignore
/// #[derive(jedem::Enum)]
/// pub enum Syntax {
///     Missing,
///     Incomplete,
///     Complete,
/// }
/// ```
#[proc_macro_derive(Enum, attributes(jedem))]
pub fn derive_enum(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
    match expand_enum(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_enum(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "#[derive(jedem::Enum)] is for enums; a struct is a record",
        ));
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new(
            input.span(),
            "an enum with no variants has no values to cross",
        ));
    }

    let mut variants = Vec::new();
    for v in &data.variants {
        if !matches!(v.fields, syn::Fields::Unit) {
            return Err(syn::Error::new(
                v.span(),
                "jedem enums carry no data -- a variant with fields is a union, \
                 which is a separate feature. Move the payload to a parameter, \
                 or keep this type out of the exported surface.",
            ));
        }
        if v.discriminant.is_some() {
            // A discriminant is fine in Rust but says nothing at the boundary,
            // where variants cross by name.
        }
        let name = v.ident.to_string();
        let wire = export_name_of(&v.attrs)?.unwrap_or_else(|| name.clone());
        // A variant has to be *nameable* in every target language, and Python
        // enum members must be identifiers. Allowing arbitrary text here would
        // generate a binding that does not compile, in one language only --
        // the worst place to discover it.
        if !is_ident(&wire) {
            return Err(syn::Error::new(
                v.span(),
                format!(
                    "`{wire}` cannot name an enum variant at the boundary: it must be a \
                     valid identifier, because some targets (Python) require one. \
                     Use letters, digits and underscores, not starting with a digit."
                ),
            ));
        }
        let doc = opt_str(doc_of(&v.attrs).as_deref());
        variants.push(quote! {
            ::jedem::Variant { name: #name, wire: #wire, doc: #doc }
        });
    }

    let ident = &input.ident;
    let name = ident.to_string();
    let doc = opt_str(doc_of(&input.attrs).as_deref());
    Ok(quote! {
        impl ::jedem::EnumType for #ident {
            const DEF: &'static ::jedem::EnumDef = &::jedem::EnumDef {
                name: #name,
                doc: #doc,
                variants: &[#(#variants),*],
            };
        }
    })
}

/// Is this a valid identifier in every target language we emit?
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Does this item carry `#[jedem(skip)]`?
fn has_skip(attrs: &[syn::Attribute]) -> syn::Result<bool> {
    for a in attrs {
        if !a.path().is_ident("jedem") {
            continue;
        }
        let mut found = false;
        // `parse_nested_meta` errors on anything it does not recognise, and
        // `name = "..."` is handled elsewhere, so tolerate it here.
        let _ = a.parse_nested_meta(|m| {
            if m.path.is_ident("skip") {
                found = true;
            }
            if m.input.peek(syn::Token![=]) {
                let _: syn::Expr = m.value()?.parse()?;
            }
            Ok(())
        });
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}
