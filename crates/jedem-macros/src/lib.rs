//! The macros. Expand to **pure data** — never to behaviour, and never to a
//! file written at expansion time.
//!
//! Use them through the `jedem` crate; this one is an implementation detail.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, FnArg, ImplItem, ItemImpl, Lit, Meta, ReturnType, Type,
};

/// Mark an `impl` block for export.
///
/// The block is emitted unchanged — the functions you wrote are the functions
/// that run — alongside a constant describing them. Drift between declaration
/// and implementation is impossible because there is only one artefact.
#[proc_macro_attribute]
pub fn export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    match expand_export(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
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
        if !matches!(f.vis, syn::Visibility::Public(_)) {
            continue;
        }
        // v1 exports free functions: an associated fn with no receiver. A
        // method needs a handle to hang itself on, which is beyond v1.
        if let Some(FnArg::Receiver(r)) = f.sig.inputs.first() {
            return Err(syn::Error::new(
                r.span(),
                "jedem v1 exports functions that take and return plain values; \
                 a method with `self` needs a handle, which is not in v1 yet. \
                 Make it an associated function, or remove it from the exported impl.",
            ));
        }
        if f.sig.asyncness.is_some() {
            return Err(syn::Error::new(
                f.sig.asyncness.span(),
                "jedem v1 is synchronous; async is not lowered yet",
            ));
        }

        let name = f.sig.ident.to_string();
        let doc = doc_of(&f.attrs);
        let export_name = export_name_of(&f.attrs)?;

        let mut params = Vec::new();
        for arg in &f.sig.inputs {
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
            params.push((pname, ty));
        }

        let (returns, fallible) = match &f.sig.output {
            ReturnType::Default => (quote!(::jedem::Type::Unit), false),
            ReturnType::Type(_, t) => match unwrap_result(t) {
                Some(inner) => (lower_type(inner)?, true),
                None => (lower_type(t)?, false),
            },
        };

        let rust_path = format!("{type_name}::{name}");
        let doc_tok = opt_str(doc.as_deref());
        let export_tok = opt_str(export_name.as_deref());
        let param_toks = params.iter().map(|(n, t)| {
            quote! { ::jedem::Param { name: #n, ty: #t } }
        });

        ops.push(quote! {
            ::jedem::Op {
                name: #name,
                doc: #doc_tok,
                export_name: #export_tok,
                params: &[#(#param_toks),*],
                returns: #returns,
                fallible: #fallible,
                rust_path: #rust_path,
            }
        });
    }

    if ops.is_empty() {
        return Err(syn::Error::new(
            input.span(),
            "#[jedem::export] found no public functions to export",
        ));
    }

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
    let types = decl.api;
    let refs = types.iter().map(|t| quote! { <#t>::JEDEM_INTERFACE });
    quote! {
        /// The jedem surface for this crate.
        pub const JEDEM_SURFACE: &'static ::jedem::Surface = &::jedem::Surface {
            name: #name,
            version: #version,
            interfaces: &[#(#refs),*],
        };
    }
    .into()
}

struct SurfaceDecl {
    name: String,
    version: String,
    api: Vec<syn::Path>,
}

impl syn::parse::Parse for SurfaceDecl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut version = None;
        let mut api = Vec::new();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![:]>()?;
            match key.to_string().as_str() {
                "name" => name = Some(input.parse::<syn::LitStr>()?.value()),
                "version" => version = Some(input.parse::<syn::LitStr>()?.value()),
                "api" => {
                    let content;
                    syn::bracketed!(content in input);
                    let list = content.parse_terminated(syn::Path::parse, syn::Token![,])?;
                    api = list.into_iter().collect();
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key `{other}`; expected name, version or api"),
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
        })
    }
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
fn unwrap_result(t: &Type) -> Option<&Type> {
    let Type::Path(p) = t else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
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
                other => Err(syn::Error::new(
                    t.span(),
                    format!(
                        "jedem cannot lower `{other}` yet. v1 handles bool, integers, f64, \
                         String/&str, Vec<u8>, Option<T> and Vec<T>. There is deliberately no \
                         fallback that would pass this across as an opaque blob."
                    ),
                )),
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
