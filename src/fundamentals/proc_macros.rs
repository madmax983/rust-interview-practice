//! # Procedural Macros: `proc_macro2`, `syn`, and `quote`
//!
//! `fundamentals::macros` drills `macro_rules!`. This module drills the other kind:
//! procedural macros — ordinary Rust functions from tokens to tokens that the compiler
//! runs at build time. Three crates do the heavy lifting:
//!
//! - **`proc_macro2`** — a `TokenStream` that also works *outside* the compiler, so the
//!   macro logic can be unit tested like any other function.
//! - **`syn`** — parses tokens into a syntax tree (`DeriveInput`, `ItemFn`, `Expr`, ...),
//!   parses helper attributes, walks/rewrites trees, and powers custom `Parse` impls.
//! - **`quote`** — turns Rust-looking templates back into tokens (`#var`, `#(#vars),*`).
//!
//! Drills in this module:
//!
//! 1. **Token trees by hand** — `Ident`/`Punct`/`Literal`/`Group`, spacing, recursion.
//! 2. **`quote!` templates** — interpolation, repetition, `format_ident!`.
//! 3. **Derive with helper attributes** — `Describe`, `#[describe(skip | rename = "..")]`.
//! 4. **Derive that generates a new type** — `Builder`, `Option`/`Vec` detection,
//!    `#[builder(each = "..", default)]`, accumulating every error at once.
//! 5. **Attribute macros that wrap a fn** — `#[retry(times = N)]` (argument parsing,
//!    `Span::mixed_site` hygiene) and `#[memoize]` (`quote_spanned!` trait assertions).
//! 6. **Rewriting a syntax tree with `VisitMut`** — `#[checked]` turns `a + b` into
//!    `a.checked_add(b)?`.
//! 7. **Function-like macro with a custom `Parse`** — `seq!(N in 0..4 { fn f~N() {} })`.
//! 8. **Testing macros** — see the tests: compare tokens, parse the output, assert errors.
//!
//! ## Why the logic lives here and not in a `proc-macro` crate
//!
//! A `proc-macro = true` crate may export *only* macros, and `proc_macro::TokenStream`
//! panics when used outside a macro expansion. The testable layout is therefore:
//!
//! ```text
//! macros/src/lib.rs            proc-macro = true   thin shims: proc_macro <-> proc_macro2
//!        |  .into()
//!        v
//! fundamentals::proc_macros    this file           all parsing + codegen on proc_macro2
//! ```
//!
//! Every macro here comes as a pair: `expand_*` returns `syn::Result<TokenStream>` (easy to
//! test), and the wrapper turns an `Err` into `compile_error!` tokens. The workspace crate
//! `rust-interview-practice-macros` (in `macros/`) exposes the wrappers as real
//! `#[derive]` / `#[attribute]` / `fn!()` macros and tests the expanded code end to end.
//!
//! ## Example
//!
//! ```
//! use quote::quote;
//! use rust_interview_practice::fundamentals::proc_macros::expand_describe;
//!
//! let input = syn::parse2(quote! {
//!     struct Point { x: i32, #[describe(rename = "why")] y: i32 }
//! })
//! .unwrap();
//! let output: syn::ItemImpl = syn::parse2(expand_describe(&input).unwrap()).unwrap();
//! assert_eq!(output.items.len(), 2); // `const NAME` and `const MEMBERS`
//! assert!(quote!(#output).to_string().contains(r#"& ["x" , "why"]"#));
//! ```
//!
//! ## Hygiene cheat sheet
//!
//! - `Span::call_site()` (what `quote!` uses) — resolves as if the user typed the tokens.
//! - `Span::mixed_site()` — local variables and labels are private to the macro (like
//!   `macro_rules!`); items and paths still resolve at the call site.
//! - Generated code names everything by absolute path (`::core::option::Option`), so a
//!   user's own `Option` or `mod core` cannot hijack it.
//! - `quote_spanned!(span=> ...)` puts type errors from generated code on the user's tokens.
//!
//! ## What a macro cannot know
//!
//! Only tokens, never types. `type Maybe<T> = Option<T>` hides an `Option` from `Builder`,
//! and `#[checked]` cannot call `checked_add` on an un-annotated integer
//! (`let x = 5; x + y` fails with E0689). Good macro errors say so up front.

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream, Parser};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{
    Attribute, BinOp, Data, DeriveInput, Expr, ExprAsync, ExprClosure, Field, Fields, FnArg,
    GenericArgument, Item, ItemFn, LitInt, LitStr, Pat, PatIdent, PatType, PathArguments,
    ReturnType, Signature, Token, Type, TypePath, braced, parse_quote,
};

// ============================================================================
// 0. Runtime support the generated code refers to
// ============================================================================

/// Implemented by `#[derive(Describe)]`: the type's name and its member names.
///
/// For structs the members are field names (tuple fields are `"0"`, `"1"`, ...);
/// for enums they are variant names.
pub trait Describe {
    /// The type's name, without generics.
    const NAME: &'static str;
    /// Field or variant names, after `#[describe(skip)]` / `#[describe(rename = "..")]`.
    const MEMBERS: &'static [&'static str];
}

/// Returned by a generated `FooBuilder::build` when a required field was never set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderError {
    /// The name of the missing field.
    pub field: &'static str,
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "missing required field `{}`", self.field)
    }
}

impl std::error::Error for BuilderError {}

/// The absolute path generated code uses to reach this module — the proc-macro
/// equivalent of `$crate`, which proc macros do not have.
fn support_path() -> TokenStream {
    quote!(::rust_interview_practice::fundamentals::proc_macros)
}

/// Folds a new error into an accumulator so a macro can report every problem at once.
fn push_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    match errors {
        Some(existing) => existing.combine(error),
        None => *errors = Some(error),
    }
}

/// Attribute-macro wrapper: on error, emit the error **and** the original item.
///
/// Dropping the item would add a cascade of "cannot find function" errors on top of the
/// real one, so well-behaved attribute macros always hand the input back.
fn or_compile_error(result: syn::Result<TokenStream>, original: TokenStream) -> TokenStream {
    result.unwrap_or_else(|error| {
        let mut tokens = error.into_compile_error();
        tokens.extend(original);
        tokens
    })
}

// ============================================================================
// 1. Token trees by hand
// ============================================================================

/// How many of each `TokenTree` kind a stream holds, counting inside groups.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TokenCounts {
    /// Identifiers and keywords (`fn`, `x`, `u8`, `self`).
    pub idents: usize,
    /// Single punctuation characters; `->` is two of them.
    pub puncts: usize,
    /// Number, string, char, and byte literals.
    pub literals: usize,
    /// Delimited groups: `( )`, `[ ]`, `{ }` and invisible groups.
    pub groups: usize,
}

/// Counts every token tree, recursing into groups.
///
/// A `TokenStream` is flat at each level: nesting exists only through `Group`.
#[must_use]
pub fn count_tokens(stream: TokenStream) -> TokenCounts {
    let mut counts = TokenCounts::default();
    count_into(stream, &mut counts);
    counts
}

fn count_into(stream: TokenStream, counts: &mut TokenCounts) {
    for tree in stream {
        match tree {
            TokenTree::Ident(_) => counts.idents += 1,
            TokenTree::Punct(_) => counts.puncts += 1,
            TokenTree::Literal(_) => counts.literals += 1,
            TokenTree::Group(group) => {
                counts.groups += 1;
                count_into(group.stream(), counts);
            }
        }
    }
}

/// Replaces every identifier spelled `from` with `to`, recursing into groups.
///
/// The replacement takes the *old* token's span, so errors still point at the user's code.
#[must_use]
pub fn replace_ident(stream: TokenStream, from: &str, to: &TokenTree) -> TokenStream {
    stream
        .into_iter()
        .map(|tree| match tree {
            TokenTree::Ident(ident) if ident == from => {
                let mut replacement = to.clone();
                replacement.set_span(ident.span());
                replacement
            }
            TokenTree::Group(group) => {
                let mut rebuilt =
                    Group::new(group.delimiter(), replace_ident(group.stream(), from, to));
                rebuilt.set_span(group.span());
                TokenTree::Group(rebuilt)
            }
            other => other,
        })
        .collect()
}

/// Builds `const NAME: u64 = VALUE;` one token at a time, without `quote!`.
///
/// `Ident::new` panics on invalid input, so the name is validated with `syn` first.
pub fn const_item_by_hand(name: &str, value: u64) -> syn::Result<TokenStream> {
    let name: Ident = syn::parse_str(name)?;
    let span = name.span();
    let tokens = [
        TokenTree::Ident(Ident::new("const", span)),
        TokenTree::Ident(name),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("u64", span)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::u64_suffixed(value)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ];
    Ok(tokens.into_iter().collect())
}

/// Wraps a stream in a delimited group, e.g. `a, b` -> `[a, b]`.
#[must_use]
pub fn wrap_in(delimiter: Delimiter, stream: TokenStream) -> TokenStream {
    TokenTree::Group(Group::new(delimiter, stream)).into()
}

// ============================================================================
// 2. quote! templates
// ============================================================================

/// Generates a fieldless enum with `ALL`, `as_str`, and `Display`.
///
/// Shows `#var` interpolation, `#(...),*` repetition over two iterators at once, and
/// interpolating a `usize` (it becomes the literal `3usize`).
pub fn string_enum(name: &str, variants: &[&str]) -> syn::Result<TokenStream> {
    let name: Ident = syn::parse_str(name)?;
    if variants.is_empty() {
        return Err(syn::Error::new(
            name.span(),
            "string_enum needs at least one variant",
        ));
    }
    let idents = variants
        .iter()
        .map(|variant| syn::parse_str::<Ident>(variant))
        .collect::<syn::Result<Vec<_>>>()?;
    let count = idents.len();
    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #name {
            #(#idents),*
        }

        impl #name {
            /// Every variant, in declaration order.
            pub const ALL: [Self; #count] = [#(Self::#idents),*];

            /// The variant's name as written.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    #(Self::#idents => #variants,)*
                }
            }
        }

        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    })
}

// ============================================================================
// 3. #[derive(Describe)] — helper attributes and generics
// ============================================================================

/// `#[derive(Describe)]` entry point: parse, expand, or emit `compile_error!`.
#[must_use]
pub fn derive_describe(input: TokenStream) -> TokenStream {
    syn::parse2::<DeriveInput>(input)
        .and_then(|input| expand_describe(&input))
        .unwrap_or_else(syn::Error::into_compile_error)
}

/// Implements [`Describe`] for a struct or enum.
///
/// `split_for_impl` turns `struct Wrapper<T: Clone> where T: Debug` into the three pieces
/// `impl<T: Clone> Describe for Wrapper<T> where T: Debug` needs.
pub fn expand_describe(input: &DeriveInput) -> syn::Result<TokenStream> {
    let mut members = Vec::new();
    match &input.data {
        Data::Struct(data) => {
            for (index, field) in data.fields.iter().enumerate() {
                let default = field
                    .ident
                    .as_ref()
                    .map_or_else(|| index.to_string(), |ident| ident.unraw().to_string());
                members.extend(describe_member(&field.attrs, default)?);
            }
        }
        Data::Enum(data) => {
            for variant in &data.variants {
                members.extend(describe_member(
                    &variant.attrs,
                    variant.ident.unraw().to_string(),
                )?);
            }
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "Describe cannot be derived for unions",
            ));
        }
    }

    let name = &input.ident;
    let name_str = name.unraw().to_string();
    let support = support_path();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #support::Describe for #name #ty_generics #where_clause {
            const NAME: &'static str = #name_str;
            const MEMBERS: &'static [&'static str] = &[#(#members),*];
        }
    })
}

/// Reads `#[describe(skip)]` / `#[describe(rename = "..")]`; `None` means skipped.
///
/// `parse_nested_meta` is the syn 2 way to read `key`, `key = value`, and `key(...)` lists;
/// `meta.error` points the message at the offending key.
fn describe_member(attrs: &[Attribute], default: String) -> syn::Result<Option<String>> {
    let mut skip = false;
    let mut rename: Option<LitStr> = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("describe")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
                Ok(())
            } else if meta.path.is_ident("rename") {
                rename = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta
                    .error("unsupported describe attribute, expected `skip` or `rename = \"...\"`"))
            }
        })?;
    }
    match (skip, rename) {
        (true, Some(rename)) => Err(syn::Error::new_spanned(
            rename,
            "`skip` and `rename` cannot be combined",
        )),
        (true, None) => Ok(None),
        (false, Some(rename)) => Ok(Some(rename.value())),
        (false, None) => Ok(Some(default)),
    }
}

// ============================================================================
// 4. #[derive(Builder)] — generating a new type
// ============================================================================

/// `#[derive(Builder)]` entry point.
#[must_use]
pub fn derive_builder(input: TokenStream) -> TokenStream {
    syn::parse2::<DeriveInput>(input)
        .and_then(|input| expand_builder(&input))
        .unwrap_or_else(syn::Error::into_compile_error)
}

/// How a field is stored, set, and finished in the generated builder.
enum BuilderKind<'a> {
    /// Plain field: `build` fails with [`BuilderError`] if it was never set.
    Required,
    /// `#[builder(default)]`: falls back to `Default::default()`.
    Defaulted,
    /// `Option<T>` field: setter takes a `T`, missing means `None`.
    Optional { inner: &'a Type },
    /// `#[builder(each = "arg")]` on `Vec<T>`: `arg(T)` pushes one element.
    Each { inner: &'a Type, setter: Ident },
}

struct BuilderField<'a> {
    ident: &'a Ident,
    ty: &'a Type,
    kind: BuilderKind<'a>,
}

/// Generates `FooBuilder` with chained setters and `build() -> Result<Foo, BuilderError>`.
///
/// Every field is classified first and *all* errors are combined, so the user fixes
/// every bad attribute in one compile instead of one per compile.
pub fn expand_builder(input: &DeriveInput) -> syn::Result<TokenStream> {
    let named = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unnamed(_) | Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Builder requires a struct with named fields",
                ));
            }
        },
        Data::Enum(data) => {
            return Err(syn::Error::new_spanned(
                data.enum_token,
                "Builder cannot be derived for enums",
            ));
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "Builder cannot be derived for unions",
            ));
        }
    };

    let mut fields = Vec::new();
    let mut errors = None;
    for field in named {
        match classify_builder_field(field) {
            Ok(field) => fields.push(field),
            Err(error) => push_error(&mut errors, error),
        }
    }
    if let Some(errors) = errors {
        return Err(errors);
    }

    let name = &input.ident;
    let vis = &input.vis;
    let builder = format_ident!("{}Builder", name);
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let storage = fields.iter().map(builder_storage);
    let init = fields.iter().map(builder_init);
    let setters = fields.iter().map(builder_setters);
    let finish = fields.iter().map(builder_finish);
    let support = support_path();

    Ok(quote! {
        #vis struct #builder #generics #where_clause {
            #(#storage,)*
        }

        impl #impl_generics #name #ty_generics #where_clause {
            /// Starts a builder with every field unset.
            #[must_use]
            #vis fn builder() -> #builder #ty_generics {
                #builder { #(#init,)* }
            }
        }

        impl #impl_generics #builder #ty_generics #where_clause {
            #(#setters)*

            /// Builds the value, failing if a required field was never set.
            #vis fn build(self) -> ::core::result::Result<#name #ty_generics, #support::BuilderError> {
                ::core::result::Result::Ok(#name { #(#finish,)* })
            }
        }
    })
}

fn classify_builder_field(field: &Field) -> syn::Result<BuilderField<'_>> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "Builder requires named fields"))?;
    let mut each: Option<LitStr> = None;
    let mut default = false;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("builder"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("each") {
                each = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("default") {
                default = true;
                Ok(())
            } else {
                Err(meta.error("expected `builder(each = \"...\")` or `builder(default)`"))
            }
        })?;
    }
    let kind = match (each, default) {
        (Some(each), true) => {
            return Err(syn::Error::new_spanned(
                each,
                "`each` and `default` cannot be combined",
            ));
        }
        (Some(each), false) => {
            let inner = generic_inner(&field.ty, "Vec").ok_or_else(|| {
                syn::Error::new_spanned(
                    &field.ty,
                    "`builder(each = ...)` requires a `Vec<T>` field",
                )
            })?;
            BuilderKind::Each {
                inner,
                setter: each.parse()?,
            }
        }
        (None, true) => BuilderKind::Defaulted,
        (None, false) => generic_inner(&field.ty, "Option")
            .map_or(BuilderKind::Required, |inner| BuilderKind::Optional {
                inner,
            }),
    };
    Ok(BuilderField {
        ident,
        ty: &field.ty,
        kind,
    })
}

/// Returns `T` if `ty` is spelled `Wrapper<T>` (any path prefix, e.g. `std::vec::Vec<T>`).
///
/// This is a *token* check: an alias like `type Maybe<T> = Option<T>` is not recognised.
#[must_use]
pub fn generic_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    if segment.ident != wrapper {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.iter().collect::<Vec<_>>().as_slice() {
        [GenericArgument::Type(inner)] => Some(inner),
        _ => None,
    }
}

fn builder_storage(field: &BuilderField<'_>) -> TokenStream {
    let BuilderField { ident, ty, kind } = field;
    match kind {
        BuilderKind::Required | BuilderKind::Defaulted => {
            quote!(#ident: ::core::option::Option<#ty>)
        }
        BuilderKind::Optional { .. } | BuilderKind::Each { .. } => quote!(#ident: #ty),
    }
}

fn builder_init(field: &BuilderField<'_>) -> TokenStream {
    let ident = field.ident;
    if matches!(field.kind, BuilderKind::Each { .. }) {
        quote!(#ident: ::std::vec::Vec::new())
    } else {
        quote!(#ident: ::core::option::Option::None)
    }
}

fn builder_setters(field: &BuilderField<'_>) -> TokenStream {
    let BuilderField { ident, ty, kind } = field;
    match kind {
        BuilderKind::Required | BuilderKind::Defaulted => quote! {
            #[must_use]
            pub fn #ident(mut self, value: impl ::core::convert::Into<#ty>) -> Self {
                self.#ident = ::core::option::Option::Some(value.into());
                self
            }
        },
        BuilderKind::Optional { inner } => quote! {
            #[must_use]
            pub fn #ident(mut self, value: impl ::core::convert::Into<#inner>) -> Self {
                self.#ident = ::core::option::Option::Some(value.into());
                self
            }
        },
        BuilderKind::Each { inner, setter } => {
            let one = quote! {
                #[must_use]
                pub fn #setter(mut self, value: impl ::core::convert::Into<#inner>) -> Self {
                    self.#ident.push(value.into());
                    self
                }
            };
            // If `each` reuses the field's name, the one-at-a-time setter wins.
            if setter == *ident {
                one
            } else {
                quote! {
                    #one

                    #[must_use]
                    pub fn #ident(mut self, values: #ty) -> Self {
                        self.#ident = values;
                        self
                    }
                }
            }
        }
    }
}

fn builder_finish(field: &BuilderField<'_>) -> TokenStream {
    let ident = field.ident;
    let name = ident.unraw().to_string();
    let support = support_path();
    match field.kind {
        BuilderKind::Required => quote! {
            #ident: self.#ident.ok_or(#support::BuilderError { field: #name })?
        },
        BuilderKind::Defaulted => quote!(#ident: self.#ident.unwrap_or_default()),
        BuilderKind::Optional { .. } | BuilderKind::Each { .. } => quote!(#ident: self.#ident),
    }
}

// ============================================================================
// 5. Attribute macros: #[retry(times = N)] and #[memoize]
// ============================================================================

/// `#[retry(times = N)]` entry point.
#[must_use]
pub fn retry(args: TokenStream, item: TokenStream) -> TokenStream {
    let result = syn::parse2::<ItemFn>(item.clone()).and_then(|item| expand_retry(args, item));
    or_compile_error(result, item)
}

/// Re-runs a `Result`-returning fn body up to `times` times until it returns `Ok`.
///
/// The body becomes `(|| -> Output { body })()`, so `?` and `return` inside it end one
/// *attempt*, not the whole function. The loop's locals use `Span::mixed_site()`, so a
/// user variable named `attempt` in the body can neither see nor clobber them.
///
/// Limitation: the body runs repeatedly, so it must not move out of its arguments.
pub fn expand_retry(args: TokenStream, item: ItemFn) -> syn::Result<TokenStream> {
    let mut times: Option<u32> = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("times") {
            let lit: LitInt = meta.value()?.parse()?;
            let value: u32 = lit.base10_parse()?;
            if value == 0 {
                return Err(syn::Error::new(lit.span(), "`times` must be at least 1"));
            }
            times = Some(value);
            Ok(())
        } else {
            Err(meta.error("unsupported retry argument, expected `times = N`"))
        }
    });
    parser.parse2(args)?;
    let times = times.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "missing `times = N`, e.g. `#[retry(times = 3)]`",
        )
    })?;

    if let Some(asyncness) = item.sig.asyncness {
        return Err(syn::Error::new_spanned(
            asyncness,
            "retry does not support async fn",
        ));
    }
    let output = match &item.sig.output {
        ReturnType::Type(_, ty) if last_segment_is(ty, "Result") => ty,
        ReturnType::Type(_, ty) => {
            return Err(syn::Error::new_spanned(
                ty,
                "retry requires a fn returning `Result<_, _>`",
            ));
        }
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &item.sig.ident,
                "retry requires a fn returning `Result<_, _>`",
            ));
        }
    };

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = &item;
    let attempt = Ident::new("attempt", Span::mixed_site());
    let result = Ident::new("result", Span::mixed_site());
    Ok(quote! {
        #(#attrs)*
        #vis #sig {
            let mut #attempt: u32 = 0;
            loop {
                #attempt += 1;
                #[allow(clippy::redundant_closure_call)]
                let #result: #output = (|| -> #output #block)();
                match #result {
                    ::core::result::Result::Err(_) if #attempt < #times => {}
                    #result => return #result,
                }
            }
        }
    })
}

/// `#[memoize]` entry point.
#[must_use]
pub fn memoize(args: TokenStream, item: TokenStream) -> TokenStream {
    let result = syn::parse2::<ItemFn>(item.clone()).and_then(|item| expand_memoize(args, item));
    or_compile_error(result, item)
}

/// Caches a one-argument fn in a per-thread `HashMap`.
///
/// The original body moves into a nested `__<name>_uncached` fn. Recursive calls in the
/// body still name the *outer* fn, so they hit the cache too (memoized `fib` is O(n)).
///
/// `quote_spanned!` attaches the `Hash + Eq + Clone` / `Clone` requirements to the user's
/// argument and return types, so a bad type is reported there instead of on `#[memoize]`.
pub fn expand_memoize(args: TokenStream, item: ItemFn) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "memoize takes no arguments"));
    }
    let sig = &item.sig;
    let (arg, pat, output) = memoize_signature(sig)?;

    let arg_ident = &pat.ident;
    let arg_ty = &arg.ty;
    let inner = format_ident!("__{}_uncached", sig.ident);
    let inner_pat = &arg.pat;
    let block = &item.block;
    let attrs = &item.attrs;
    let vis = &item.vis;

    // The outer fn never mutates its argument, so drop any `mut` to avoid `unused_mut`.
    let mut outer_sig = sig.clone();
    if let Some(FnArg::Typed(outer_arg)) = outer_sig.inputs.first_mut()
        && let Pat::Ident(outer_pat) = &mut *outer_arg.pat
    {
        outer_pat.mutability = None;
    }

    // The cache logic lives in a generic helper, so its `HashMap` calls type-check once,
    // generically. A bad user type then fails a *bound* at a call site we control, and
    // `quote_spanned!` puts that call site on the user's own tokens: `#[memoize] fn
    // f(x: f32) -> f32` reports "`f32: Hash` is not satisfied" pointing at the argument
    // type, and a non-`Clone` output type is reported on the return type.
    let body = quote_spanned! {Span::mixed_site()=>
        fn cached<K, V>(
            cache: &'static ::std::thread::LocalKey<
                ::core::cell::RefCell<::std::collections::HashMap<K, V>>,
            >,
            key: K,
            compute: fn(K) -> V,
            clone_value: fn(&V) -> V,
        ) -> V
        where
            K: ::core::hash::Hash + ::core::cmp::Eq + ::core::clone::Clone,
        {
            if let ::core::option::Option::Some(hit) =
                cache.with(|cache| cache.borrow().get(&key).map(clone_value))
            {
                return hit;
            }
            let value = compute(::core::clone::Clone::clone(&key));
            cache.with(|cache| cache.borrow_mut().insert(key, clone_value(&value)));
            value
        }
        ::std::thread_local! {
            static CACHE: ::core::cell::RefCell<::std::collections::HashMap<#arg_ty, #output>> =
                ::core::cell::RefCell::new(::std::collections::HashMap::new());
        }
    };
    let clone_value = quote_spanned! {output.span()=> <#output as ::core::clone::Clone>::clone };
    let call = quote_spanned! {arg_ty.span()=> cached(&CACHE, #arg_ident, #inner, #clone_value) };
    Ok(quote! {
        #(#attrs)*
        #vis #outer_sig {
            fn #inner(#inner_pat: #arg_ty) -> #output #block
            #body
            #call
        }
    })
}

/// Checks `#[memoize]`'s signature rules and returns the argument, its identifier
/// pattern, and the output type.
fn memoize_signature(sig: &Signature) -> syn::Result<(&PatType, &PatIdent, &Type)> {
    if let Some(asyncness) = sig.asyncness {
        return Err(syn::Error::new_spanned(
            asyncness,
            "memoize does not support async fn",
        ));
    }
    if !sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &sig.generics,
            "memoize does not support generic fns (the cache is one `static` per fn)",
        ));
    }
    let ReturnType::Type(_, output) = &sig.output else {
        return Err(syn::Error::new_spanned(
            &sig.ident,
            "memoize requires a return value to cache",
        ));
    };
    let mut inputs = sig.inputs.iter();
    let (Some(input), None) = (inputs.next(), inputs.next()) else {
        return Err(syn::Error::new_spanned(
            &sig.inputs,
            "memoize requires exactly one argument",
        ));
    };
    let arg = match input {
        FnArg::Typed(arg) => arg,
        FnArg::Receiver(receiver) => {
            return Err(syn::Error::new_spanned(
                receiver,
                "memoize does not support methods",
            ));
        }
    };
    let Pat::Ident(pat) = &*arg.pat else {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "memoize requires a plain identifier argument",
        ));
    };
    if pat.by_ref.is_some() || pat.subpat.is_some() {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "memoize requires a plain identifier argument",
        ));
    }

    if let Type::Reference(reference) = &*arg.ty {
        return Err(syn::Error::new_spanned(
            reference,
            "memoize needs an owned argument type because the cache outlives every call \
             (e.g. take `String` instead of `&str`)",
        ));
    }
    Ok((arg, pat, output))
}

fn last_segment_is(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(TypePath { path, .. }) if path.segments.last().is_some_and(|s| s.ident == name))
}

// ============================================================================
// 6. Rewriting a syntax tree with VisitMut: #[checked]
// ============================================================================

/// `#[checked]` entry point.
#[must_use]
pub fn checked(args: TokenStream, item: TokenStream) -> TokenStream {
    let result = syn::parse2::<ItemFn>(item.clone()).and_then(|item| expand_checked(args, item));
    or_compile_error(result, item)
}

/// Rewrites `+ - *` (and `+= -= *=`) in an `Option`-returning fn into checked arithmetic
/// that short-circuits with `?`: `a + b * c` becomes
/// `(a).checked_add((b).checked_mul(c)?)?`.
///
/// Closures, `async` blocks, and nested items are left alone — a `?` inside them would
/// target *their* return type, not this fn's. Macro arguments (`assert_eq!(a + b, ..)`)
/// are opaque tokens to `syn` and are not rewritten either.
pub fn expand_checked(args: TokenStream, mut item: ItemFn) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "checked takes no arguments"));
    }
    match &item.sig.output {
        ReturnType::Type(_, ty) if generic_inner(ty, "Option").is_some() => {}
        ReturnType::Type(_, ty) => {
            return Err(syn::Error::new_spanned(
                ty,
                "checked requires a fn returning `Option<_>` so overflow can short-circuit with `?`",
            ));
        }
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &item.sig.ident,
                "checked requires a fn returning `Option<_>` so overflow can short-circuit with `?`",
            ));
        }
    }
    let mut rewriter = CheckedArithmetic::default();
    rewriter.visit_block_mut(&mut item.block);
    rewriter
        .errors
        .map_or_else(|| Ok(item.into_token_stream()), Err)
}

/// A `VisitMut` can't return `Result`, so errors are collected and reported at the end.
#[derive(Default)]
struct CheckedArithmetic {
    errors: Option<syn::Error>,
}

impl VisitMut for CheckedArithmetic {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // Post-order: rewrite the operands first so nested arithmetic is checked too.
        visit_mut::visit_expr_mut(self, expr);
        let Expr::Binary(binary) = expr else {
            return;
        };
        let (method, compound, commutative) = match binary.op {
            BinOp::Add(_) => ("checked_add", false, true),
            BinOp::Sub(_) => ("checked_sub", false, false),
            BinOp::Mul(_) => ("checked_mul", false, true),
            BinOp::AddAssign(_) => ("checked_add", true, false),
            BinOp::SubAssign(_) => ("checked_sub", true, false),
            BinOp::MulAssign(_) => ("checked_mul", true, false),
            _ => return,
        };
        let method = Ident::new(method, binary.op.span());
        let (mut left, mut right) = (&binary.left, &binary.right);

        if compound {
            // `v[next()] += 1` would evaluate `next()` twice once expanded.
            if !matches!(**left, Expr::Path(_)) {
                push_error(
                    &mut self.errors,
                    syn::Error::new_spanned(
                        left,
                        "checked compound assignment needs a plain variable on the left",
                    ),
                );
                return;
            }
            *expr = parse_quote!(#left = (#left).#method(#right)?);
            return;
        }

        // `2.checked_mul(x)` is E0689 (ambiguous numeric type): swap when the op allows.
        if is_unsuffixed_int(left) {
            if commutative {
                (left, right) = (right, left);
            } else {
                push_error(
                    &mut self.errors,
                    syn::Error::new_spanned(
                        left,
                        "checked arithmetic needs a typed left operand; add a suffix like `10u32`",
                    ),
                );
                return;
            }
        }
        *expr = parse_quote!((#left).#method(#right)?);
    }

    fn visit_expr_closure_mut(&mut self, _closure: &mut ExprClosure) {}

    fn visit_expr_async_mut(&mut self, _block: &mut ExprAsync) {}

    fn visit_item_mut(&mut self, _item: &mut Item) {}
}

fn is_unsuffixed_int(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(lit) => matches!(&lit.lit, syn::Lit::Int(int) if int.suffix().is_empty()),
        Expr::Paren(paren) => is_unsuffixed_int(&paren.expr),
        Expr::Group(group) => is_unsuffixed_int(&group.expr),
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && is_unsuffixed_int(&unary.expr)
        }
        _ => false,
    }
}

// ============================================================================
// 7. Function-like macro with a custom Parse: seq!
// ============================================================================

/// The most copies `seq!` will generate before refusing.
pub const MAX_SEQ_LEN: u64 = 4096;

/// Parsed `seq!(N in START..END { body })` (also `START..=END`).
#[derive(Debug, Clone)]
pub struct SeqInput {
    /// The loop variable, replaced by each value in the body.
    pub var: Ident,
    /// First value (inclusive).
    pub start: u64,
    /// One past the last value (exclusive), whichever range syntax was written.
    pub end: u64,
    /// The body tokens, copied once per value.
    pub body: TokenStream,
}

impl Parse for SeqInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let var: Ident = input.parse()?;
        input.parse::<Token![in]>()?;
        let start_lit: LitInt = input.parse()?;
        // `lookahead1` collects what was tried, giving "expected `..=` or `..`" for free.
        // `..=` is checked first because `..` would also match its first two characters.
        let lookahead = input.lookahead1();
        let inclusive = if lookahead.peek(Token![..=]) {
            input.parse::<Token![..=]>()?;
            true
        } else if lookahead.peek(Token![..]) {
            input.parse::<Token![..]>()?;
            false
        } else {
            return Err(lookahead.error());
        };
        let end_lit: LitInt = input.parse()?;
        let content;
        braced!(content in input);
        let body: TokenStream = content.parse()?;

        let start: u64 = start_lit.base10_parse()?;
        let end: u64 = end_lit.base10_parse()?;
        let end = if inclusive {
            end.checked_add(1)
                .ok_or_else(|| syn::Error::new(end_lit.span(), "range end overflows u64"))?
        } else {
            end
        };
        if start > end {
            return Err(syn::Error::new(
                start_lit.span(),
                "seq! range start is after its end",
            ));
        }
        if end - start > MAX_SEQ_LEN {
            return Err(syn::Error::new(
                end_lit.span(),
                format!("seq! would generate more than {MAX_SEQ_LEN} copies"),
            ));
        }
        Ok(Self {
            var,
            start,
            end,
            body,
        })
    }
}

/// `seq!` entry point.
#[must_use]
pub fn seq(input: TokenStream) -> TokenStream {
    syn::parse2::<SeqInput>(input)
        .map_or_else(syn::Error::into_compile_error, |input| expand_seq(&input))
}

/// Copies the body once per value, replacing `N` with the value and pasting `name~N`
/// into a single identifier (`name0`, `name1`, ...).
///
/// `macro_rules!` can't do either: it can't count, and it can't build new identifiers.
#[must_use]
pub fn expand_seq(input: &SeqInput) -> TokenStream {
    (input.start..input.end)
        .map(|value| substitute(input.body.clone(), &input.var, value))
        .collect()
}

fn substitute(stream: TokenStream, var: &Ident, value: u64) -> TokenStream {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        match (&tokens[index], tokens.get(index + 1), tokens.get(index + 2)) {
            (
                TokenTree::Ident(prefix),
                Some(TokenTree::Punct(tilde)),
                Some(TokenTree::Ident(suffix)),
            ) if tilde.as_char() == '~' && suffix == var => {
                out.push(TokenTree::Ident(format_ident!(
                    "{}{}",
                    prefix,
                    value,
                    span = prefix.span()
                )));
                index += 3;
                continue;
            }
            (TokenTree::Ident(ident), _, _) if ident == var => {
                let mut literal = Literal::u64_unsuffixed(value);
                literal.set_span(ident.span());
                out.push(TokenTree::Literal(literal));
            }
            (TokenTree::Group(group), _, _) => {
                let mut rebuilt =
                    Group::new(group.delimiter(), substitute(group.stream(), var, value));
                rebuilt.set_span(group.span());
                out.push(TokenTree::Group(rebuilt));
            }
            (other, _, _) => out.push(other.clone()),
        }
        index += 1;
    }
    out.into_iter().collect()
}

// ============================================================================
// 8. Testing macros
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{ItemImpl, ItemStruct};

    /// Token streams have no `PartialEq`; their `Display` form is the stable comparison.
    fn assert_tokens_eq(actual: &TokenStream, expected: &TokenStream) {
        assert_eq!(actual.to_string(), expected.to_string());
    }

    /// The first error message, as the user would see it.
    fn error_message<T>(result: syn::Result<T>) -> String {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(error) => error.to_string(),
        }
    }

    fn item_fn(tokens: TokenStream) -> ItemFn {
        syn::parse2(tokens).expect("valid fn")
    }

    fn derive_input(tokens: TokenStream) -> DeriveInput {
        syn::parse2(tokens).expect("valid derive input")
    }

    // ---- 1. token trees -------------------------------------------------

    #[test]
    fn count_tokens_recurses_into_groups_and_splits_multi_char_puncts() {
        let counts = count_tokens(quote!(
            fn add(a: u8) -> u8 {
                a + 1
            }
        ));
        assert_eq!(
            counts,
            TokenCounts {
                idents: 6, // fn add a u8 u8 a
                puncts: 4, // : - > +
                literals: 1,
                groups: 2, // ( ) and { }
            }
        );
    }

    #[test]
    fn multi_char_operators_are_joint_puncts() {
        let puncts: Vec<(char, Spacing)> = quote!(->)
            .into_iter()
            .filter_map(|tree| match tree {
                TokenTree::Punct(punct) => Some((punct.as_char(), punct.spacing())),
                _ => None,
            })
            .collect();
        assert_eq!(puncts, [('-', Spacing::Joint), ('>', Spacing::Alone)]);
    }

    #[test]
    fn lifetimes_are_a_joint_quote_then_an_ident() {
        let counts = count_tokens(quote!('a));
        assert_eq!((counts.puncts, counts.idents), (1, 1));
    }

    #[test]
    fn count_tokens_of_empty_stream_is_zero() {
        assert_eq!(count_tokens(TokenStream::new()), TokenCounts::default());
    }

    #[test]
    fn replace_ident_reaches_nested_groups() {
        let to = TokenTree::Ident(Ident::new("y", Span::call_site()));
        let out = replace_ident(quote!(x + f(x, [x]) + xx), "x", &to);
        assert_tokens_eq(&out, &quote!(y + f(y, [y]) + xx));
    }

    #[test]
    fn replace_ident_can_substitute_a_literal() {
        let to = TokenTree::Literal(Literal::u8_unsuffixed(7));
        assert_tokens_eq(&replace_ident(quote!(N * N), "N", &to), &quote!(7 * 7));
    }

    #[test]
    fn const_item_by_hand_matches_quote() {
        let by_hand = const_item_by_hand("MAX", 10).expect("valid name");
        assert_tokens_eq(
            &by_hand,
            &quote!(
                const MAX: u64 = 10u64;
            ),
        );
        let item: syn::ItemConst = syn::parse2(by_hand).expect("parses as a const");
        assert_eq!(item.ident, "MAX");
    }

    #[test]
    fn const_item_by_hand_rejects_invalid_names_instead_of_panicking() {
        assert!(const_item_by_hand("1abc", 1).is_err());
        assert!(const_item_by_hand("fn", 1).is_err());
        assert!(const_item_by_hand("", 1).is_err());
    }

    #[test]
    fn wrap_in_builds_a_group() {
        assert_tokens_eq(&wrap_in(Delimiter::Bracket, quote!(a, b)), &quote!([a, b]));
    }

    // ---- 2. quote! ------------------------------------------------------

    #[test]
    fn string_enum_generates_enum_and_impls() {
        let out = string_enum("Color", &["Red", "Green", "Blue"]).expect("valid");
        let file: syn::File = syn::parse2(out).expect("parses as items");
        assert_eq!(file.items.len(), 3);
        let Item::Enum(item) = &file.items[0] else {
            panic!("first item is the enum")
        };
        let names: Vec<String> = item.variants.iter().map(|v| v.ident.to_string()).collect();
        assert_eq!(names, ["Red", "Green", "Blue"]);
        let text = quote!(#file).to_string();
        assert!(text.contains("[Self ; 3usize]"), "{text}");
        assert!(text.contains(r#"Self :: Green => "Green""#), "{text}");
    }

    #[test]
    fn string_enum_rejects_keywords_and_empty_lists() {
        assert!(
            error_message(string_enum("Color", &["Red", "fn"])).contains("expected identifier")
        );
        assert!(error_message(string_enum("Color", &[])).contains("at least one variant"));
    }

    // ---- 3. Describe ----------------------------------------------------

    #[test]
    fn describe_named_struct() {
        let input = derive_input(quote! { struct Point { x: i32, y: i32 } });
        let expected = quote! {
            impl ::rust_interview_practice::fundamentals::proc_macros::Describe for Point {
                const NAME: &'static str = "Point";
                const MEMBERS: &'static [&'static str] = &["x", "y"];
            }
        };
        assert_tokens_eq(&expand_describe(&input).expect("expands"), &expected);
    }

    #[test]
    fn describe_tuple_unit_and_enum() {
        let tuple = expand_describe(&derive_input(quote! { struct Pair(u8, u8); })).expect("ok");
        assert!(tuple.to_string().contains(r#"& ["0" , "1"]"#));
        let unit = expand_describe(&derive_input(quote! { struct Unit; })).expect("ok");
        assert!(unit.to_string().contains("& []"));
        let en = expand_describe(&derive_input(quote! { enum E { A, B(u8), C { x: u8 } } }))
            .expect("ok");
        assert!(en.to_string().contains(r#"& ["A" , "B" , "C"]"#));
    }

    #[test]
    fn describe_handles_helper_attributes_and_raw_idents() {
        let input = derive_input(quote! {
            struct S {
                #[describe(rename = "kind")] r#type: u8,
                #[describe(skip)] secret: u8,
                #[doc = "other attributes are ignored"] plain: u8,
            }
        });
        let out = expand_describe(&input).expect("expands");
        assert!(out.to_string().contains(r#"& ["kind" , "plain"]"#), "{out}");
    }

    #[test]
    fn describe_keeps_generics_and_where_clauses() {
        let input = derive_input(quote! { struct W<'a, T: Clone> where T: Default { r: &'a T } });
        let item: ItemImpl = syn::parse2(expand_describe(&input).expect("expands")).expect("impl");
        assert_eq!(item.generics.params.len(), 2);
        assert!(item.generics.where_clause.is_some());
        let self_ty = &item.self_ty;
        assert_tokens_eq(&quote!(#self_ty), &quote!(W<'a, T>));
    }

    #[test]
    fn describe_errors() {
        let union = derive_input(quote! { union U { a: u8 } });
        assert!(error_message(expand_describe(&union)).contains("unions"));
        let unknown = derive_input(quote! { struct S { #[describe(hide)] a: u8 } });
        assert!(
            error_message(expand_describe(&unknown)).contains("unsupported describe attribute")
        );
        let both = derive_input(quote! { struct S { #[describe(skip, rename = "b")] a: u8 } });
        assert!(error_message(expand_describe(&both)).contains("cannot be combined"));
        let not_str = derive_input(quote! { struct S { #[describe(rename = 3)] a: u8 } });
        assert!(error_message(expand_describe(&not_str)).contains("expected string literal"));
    }

    #[test]
    fn derive_describe_turns_errors_into_compile_error() {
        let out = derive_describe(quote! { union U { a: u8 } });
        assert!(
            out.to_string().starts_with(":: core :: compile_error !"),
            "{out}"
        );
        let garbage = derive_describe(quote! { not an item });
        assert!(garbage.to_string().contains("compile_error"));
    }

    // ---- 4. Builder -----------------------------------------------------

    fn builder_items(tokens: TokenStream) -> (ItemStruct, Vec<ItemImpl>) {
        let out = expand_builder(&derive_input(tokens)).expect("expands");
        let file: syn::File = syn::parse2(out).expect("parses");
        let mut items = file.items.into_iter();
        let Some(Item::Struct(builder)) = items.next() else {
            panic!("builder struct first")
        };
        let impls = items
            .map(|item| match item {
                Item::Impl(item) => item,
                other => panic!("unexpected item {other:?}"),
            })
            .collect();
        (builder, impls)
    }

    fn method_names(item: &ItemImpl) -> Vec<String> {
        item.items
            .iter()
            .filter_map(|item| match item {
                syn::ImplItem::Fn(f) => Some(f.sig.ident.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn builder_generates_storage_setters_and_build() {
        let (builder, impls) = builder_items(quote! {
            pub struct Command {
                executable: String,
                #[builder(each = "arg")] args: Vec<String>,
                current_dir: Option<String>,
                #[builder(default)] retries: u8,
            }
        });
        assert_eq!(builder.ident, "CommandBuilder");
        let types: Vec<String> = builder
            .fields
            .iter()
            .map(|f| f.ty.to_token_stream().to_string())
            .collect();
        assert_eq!(
            types,
            [
                ":: core :: option :: Option < String >",
                "Vec < String >",
                "Option < String >",
                ":: core :: option :: Option < u8 >",
            ]
        );
        assert_eq!(method_names(&impls[0]), ["builder"]);
        assert_eq!(
            method_names(&impls[1]),
            [
                "executable",
                "arg",
                "args",
                "current_dir",
                "retries",
                "build"
            ]
        );
        let build = impls[1].to_token_stream().to_string();
        assert!(
            build.contains(r#"BuilderError { field : "executable" }"#),
            "{build}"
        );
        assert!(
            build.contains("retries : self . retries . unwrap_or_default ()"),
            "{build}"
        );
    }

    #[test]
    fn builder_each_with_the_field_name_skips_the_whole_vec_setter() {
        let (_, impls) =
            builder_items(quote! { struct S { #[builder(each = "items")] items: Vec<u8> } });
        assert_eq!(method_names(&impls[1]), ["items", "build"]);
    }

    #[test]
    fn builder_carries_generics_into_the_builder() {
        let (builder, impls) =
            builder_items(quote! { struct S<'a, T> where T: Clone { r: &'a T } });
        assert_eq!(builder.generics.params.len(), 2);
        assert!(builder.generics.where_clause.is_some());
        let self_ty = &impls[1].self_ty;
        assert_tokens_eq(&quote!(#self_ty), &quote!(SBuilder<'a, T>));
    }

    #[test]
    fn builder_rejects_non_named_structs() {
        for input in [
            quote! { enum E { A } },
            quote! { struct T(u8); },
            quote! { struct U; },
            quote! { union X { a: u8 } },
        ] {
            assert!(expand_builder(&derive_input(input)).is_err());
        }
    }

    #[test]
    fn builder_attribute_errors() {
        let not_vec = derive_input(quote! { struct S { #[builder(each = "x")] xs: Option<u8> } });
        assert!(error_message(expand_builder(&not_vec)).contains("requires a `Vec<T>` field"));
        let unknown = derive_input(quote! { struct S { #[builder(eac = "x")] xs: Vec<u8> } });
        assert!(
            error_message(expand_builder(&unknown)).contains("expected `builder(each = \"...\")`")
        );
        let both =
            derive_input(quote! { struct S { #[builder(each = "x", default)] xs: Vec<u8> } });
        assert!(error_message(expand_builder(&both)).contains("cannot be combined"));
        let bad_ident =
            derive_input(quote! { struct S { #[builder(each = "not an ident")] xs: Vec<u8> } });
        assert!(expand_builder(&bad_ident).is_err());
    }

    #[test]
    fn builder_reports_every_bad_field_at_once() {
        let input = derive_input(quote! {
            struct S {
                #[builder(nope)] a: u8,
                ok: u8,
                #[builder(each = "b")] b: u8,
                #[builder(also_nope)] c: u8,
            }
        });
        let Err(errors) = expand_builder(&input) else {
            panic!("expected errors")
        };
        assert_eq!(errors.into_iter().count(), 3);
    }

    #[test]
    fn generic_inner_matches_only_single_argument_wrappers() {
        let ty: Type = parse_quote!(::std::option::Option<Vec<u8>>);
        let inner = generic_inner(&ty, "Option").expect("is an Option");
        assert_tokens_eq(&quote!(#inner), &quote!(Vec<u8>));
        let result: Type = parse_quote!(Result<u8, String>);
        assert!(generic_inner(&result, "Result").is_none());
        let alias: Type = parse_quote!(Maybe<u8>);
        assert!(generic_inner(&alias, "Option").is_none());
        let bare: Type = parse_quote!(Option);
        assert!(generic_inner(&bare, "Option").is_none());
        let reference: Type = parse_quote!(&Option<u8>);
        assert!(generic_inner(&reference, "Option").is_none());
    }

    // ---- 5. attribute macros --------------------------------------------

    #[test]
    fn retry_wraps_the_body_in_a_loop() {
        let out = expand_retry(
            quote!(times = 3),
            item_fn(quote! { pub fn fetch(n: u8) -> Result<u8, String> { Ok(n) } }),
        )
        .expect("expands");
        let item: ItemFn = syn::parse2(out).expect("still a fn");
        assert_eq!(item.sig.ident, "fetch");
        let text = item.block.to_token_stream().to_string();
        assert!(text.contains("loop"), "{text}");
        assert!(text.contains("< 3u32"), "{text}");
        // syn prints a reparsed `||` closure as `| |`.
        assert!(
            text.contains("(| | -> Result < u8 , String > { Ok (n) }) ()"),
            "{text}"
        );
    }

    #[test]
    fn retry_argument_errors() {
        let ok_fn = || item_fn(quote! { fn f() -> Result<(), ()> { Ok(()) } });
        assert!(error_message(expand_retry(quote!(), ok_fn())).contains("missing `times = N`"));
        assert!(error_message(expand_retry(quote!(times = 0), ok_fn())).contains("at least 1"));
        assert!(
            error_message(expand_retry(quote!(tries = 2), ok_fn()))
                .contains("unsupported retry argument")
        );
        assert!(expand_retry(quote!(times = "3"), ok_fn()).is_err());
        assert!(expand_retry(quote!(times = 99999999999), ok_fn()).is_err());
    }

    #[test]
    fn retry_signature_errors() {
        let three = || quote!(times = 3);
        let no_result = item_fn(quote! { fn f() -> u8 { 1 } });
        assert!(error_message(expand_retry(three(), no_result)).contains("returning `Result"));
        let unit = item_fn(quote! { fn f() {} });
        assert!(error_message(expand_retry(three(), unit)).contains("returning `Result"));
        let async_fn = item_fn(quote! { async fn f() -> Result<(), ()> { Ok(()) } });
        assert!(error_message(expand_retry(three(), async_fn)).contains("async"));
    }

    #[test]
    fn retry_wrapper_keeps_the_original_item_on_error() {
        let out = retry(quote!(), quote! { fn keep() -> Result<(), ()> { Ok(()) } }).to_string();
        assert!(out.contains("compile_error"), "{out}");
        assert!(out.contains("fn keep"), "{out}");
    }

    #[test]
    fn memoize_moves_the_body_into_an_inner_fn() {
        let out = expand_memoize(
            TokenStream::new(),
            item_fn(quote! { fn fib(mut n: u64) -> u64 { n } }),
        )
        .expect("expands");
        let item: ItemFn = syn::parse2(out).expect("still a fn");
        let outer_arg = item.sig.inputs.to_token_stream().to_string();
        assert_eq!(outer_arg, "n : u64", "outer `mut` is dropped");
        let Some(syn::Stmt::Item(Item::Fn(inner))) = item.block.stmts.first() else {
            panic!("first statement is the inner fn");
        };
        assert_eq!(inner.sig.ident, "__fib_uncached");
        assert_eq!(
            inner.sig.inputs.to_token_stream().to_string(),
            "mut n : u64"
        );
        assert!(
            item.block
                .to_token_stream()
                .to_string()
                .contains("thread_local")
        );
    }

    #[test]
    fn memoize_errors() {
        let m = |tokens| expand_memoize(TokenStream::new(), item_fn(tokens));
        assert!(error_message(m(quote! { fn f<T>(t: T) -> u8 { 1 } })).contains("generic"));
        assert!(
            error_message(m(quote! { fn f(a: u8, b: u8) -> u8 { a } })).contains("exactly one")
        );
        assert!(error_message(m(quote! { fn f() -> u8 { 1 } })).contains("exactly one"));
        assert!(error_message(m(quote! { fn f(a: u8) { } })).contains("return value"));
        assert!(error_message(m(quote! { fn f(&self) -> u8 { 1 } })).contains("methods"));
        assert!(
            error_message(m(quote! { fn f((a, b): (u8, u8)) -> u8 { a } }))
                .contains("plain identifier")
        );
        assert!(
            error_message(m(quote! { fn f(ref a: u8) -> u8 { 1 } })).contains("plain identifier")
        );
        assert!(error_message(m(quote! { async fn f(a: u8) -> u8 { a } })).contains("async"));
        assert!(
            error_message(m(quote! { fn f(s: &str) -> usize { s.len() } }))
                .contains("owned argument")
        );
        let args = expand_memoize(
            quote!(size = 3),
            item_fn(quote! { fn f(a: u8) -> u8 { a } }),
        );
        assert!(error_message(args).contains("no arguments"));
    }

    // ---- 6. checked -----------------------------------------------------

    fn checked_body(tokens: TokenStream) -> String {
        let out = expand_checked(TokenStream::new(), item_fn(tokens)).expect("expands");
        let item: ItemFn = syn::parse2(out).expect("still a fn");
        item.block.to_token_stream().to_string()
    }

    #[test]
    fn checked_rewrites_nested_arithmetic_post_order() {
        let body =
            checked_body(quote! { fn f(a: u8, b: u8, c: u8) -> Option<u8> { Some(a + b * c) } });
        let expected = quote!({ Some((a).checked_add((b).checked_mul(c)?)?) });
        assert_eq!(body, expected.to_string());
    }

    #[test]
    fn checked_leaves_other_operators_alone() {
        let body =
            checked_body(quote! { fn f(a: u8, b: u8) -> Option<bool> { Some(a / b == a % b) } });
        assert_eq!(body, quote!({ Some(a / b == a % b) }).to_string());
    }

    #[test]
    fn checked_rewrites_compound_assignment() {
        let body = checked_body(
            quote! { fn f(mut a: u8) -> Option<u8> { a += 1; a -= 2; a *= 3; Some(a) } },
        );
        let expected = quote!({
            a = (a).checked_add(1)?;
            a = (a).checked_sub(2)?;
            a = (a).checked_mul(3)?;
            Some(a)
        });
        assert_eq!(body, expected.to_string());
    }

    #[test]
    fn checked_swaps_a_literal_left_operand_when_commutative() {
        let body = checked_body(quote! { fn f(a: u8) -> Option<u8> { Some(2 * a + 1) } });
        assert_eq!(
            body,
            quote!({ Some(((a).checked_mul(2)?).checked_add(1)?) }).to_string()
        );
    }

    #[test]
    fn checked_skips_closures_async_blocks_and_nested_items() {
        let body = checked_body(quote! {
            fn f(a: u8) -> Option<u8> {
                let g = |x: u8| x + 1;
                let h = async { a + 1 };
                fn inner(x: u8) -> u8 { x + 1 }
                Some(a)
            }
        });
        assert!(!body.contains("checked"), "{body}");
    }

    #[test]
    fn checked_errors() {
        let c = |tokens| expand_checked(TokenStream::new(), item_fn(tokens));
        assert!(
            error_message(c(quote! { fn f(a: u8) -> u8 { a + 1 } }))
                .contains("returning `Option<_>`")
        );
        assert!(error_message(c(quote! { fn f(a: u8) { } })).contains("returning `Option<_>`"));
        assert!(
            error_message(c(quote! { fn f(a: u8) -> Option<u8> { Some(10 - a) } }))
                .contains("suffix")
        );
        assert!(
            error_message(c(quote! { fn f(a: u8) -> Option<u8> { Some(-(1) - a) } }))
                .contains("suffix")
        );
        let place = c(quote! { fn f(mut v: [u8; 2]) -> Option<u8> { v[0] += 1; Some(v[0]) } });
        assert!(error_message(place).contains("plain variable"));
        let args = expand_checked(
            quote!(wrapping),
            item_fn(quote! { fn f() -> Option<u8> { None } }),
        );
        assert!(error_message(args).contains("no arguments"));
    }

    #[test]
    fn checked_collects_every_error() {
        let Err(errors) = expand_checked(
            TokenStream::new(),
            item_fn(
                quote! { fn f(a: u8, mut v: [u8; 1]) -> Option<u8> { v[0] += 1; Some(1 - a) } },
            ),
        ) else {
            panic!("expected errors");
        };
        assert_eq!(errors.into_iter().count(), 2);
    }

    #[test]
    fn checked_suffixed_literal_on_the_left_is_fine() {
        let body = checked_body(quote! { fn f(a: u8) -> Option<u8> { Some(10u8 - a) } });
        assert_eq!(body, quote!({ Some((10u8).checked_sub(a)?) }).to_string());
    }

    // ---- 7. seq! --------------------------------------------------------

    fn parse_seq(tokens: TokenStream) -> syn::Result<SeqInput> {
        syn::parse2(tokens)
    }

    #[test]
    fn seq_parses_exclusive_and_inclusive_ranges() {
        let exclusive = parse_seq(quote!(N in 1..4 { N })).expect("parses");
        assert_eq!(
            (
                exclusive.var.to_string().as_str(),
                exclusive.start,
                exclusive.end
            ),
            ("N", 1, 4)
        );
        let inclusive = parse_seq(quote!(N in 1..=4 { N })).expect("parses");
        assert_eq!((inclusive.start, inclusive.end), (1, 5));
        let empty = parse_seq(quote!(N in 3..3 {})).expect("parses");
        assert!(expand_seq(&empty).is_empty());
    }

    #[test]
    fn seq_substitutes_and_pastes_identifiers() {
        let input = parse_seq(quote!(N in 0..3 { fn get~N() -> u64 { N * 10 } })).expect("parses");
        let expected = quote! {
            fn get0() -> u64 { 0 * 10 }
            fn get1() -> u64 { 1 * 10 }
            fn get2() -> u64 { 2 * 10 }
        };
        assert_tokens_eq(&expand_seq(&input), &expected);
    }

    #[test]
    fn seq_only_replaces_the_exact_identifier() {
        let input = parse_seq(quote!(N in 5..6 { NN + N + n })).expect("parses");
        assert_tokens_eq(&expand_seq(&input), &quote!(NN + 5 + n));
    }

    #[test]
    fn seq_parse_errors() {
        assert!(error_message(parse_seq(quote!(N in 0-3 {}))).contains("expected `..=` or `..`"));
        assert!(error_message(parse_seq(quote!(N in 0 3 {}))).contains("expected `..=` or `..`"));
        assert!(error_message(parse_seq(quote!(N 0..3 {}))).contains("expected `in`"));
        assert!(error_message(parse_seq(quote!(N in 4..2 {}))).contains("start is after its end"));
        assert!(error_message(parse_seq(quote!(N in 0..5000 {}))).contains("more than 4096"));
        assert!(
            error_message(parse_seq(quote!(N in 0..=18446744073709551615 {})))
                .contains("overflows")
        );
        assert!(
            parse_seq(quote!(N in 0..3 (N))).is_err(),
            "body must be braced"
        );
        assert!(
            parse_seq(quote!(N in 0..3 {} extra)).is_err(),
            "trailing tokens rejected"
        );
    }

    #[test]
    fn seq_wrapper_emits_compile_error() {
        assert!(
            seq(quote!(N in 3..1 {}))
                .to_string()
                .contains("compile_error")
        );
    }
}
