//! `#[macro_pub]` is a replacement for `#[macro_export]` which provides normal
//! visibility rules for `macro_rules!` macros.
//!
//! If you want the macro to not have world-public visibility, then use
//! `pub(in path)` syntax in the attribute, e.g. `#[macro_pub(crate)]`.
//!
//! # How
//!
//! If you `use` a legacy scoped macro, it "mounts" it to the normal visibility
//! and can be used as a normal item from then on. So when you write
//!
//! ```
//! # use macro_pub::macro_pub;
//! #[macro_pub(crate)]
//! macro_rules! my_macro {
//!     () => {};
//! }
//! ```
//!
//! it expands to something like
//!
//! ```
//! macro_rules! macro_impl_279137529572831871236407390024221977230_my_macro {
//!     () => {};
//! }
//! pub(crate) use macro_impl_279137529572831871236407390024221977230_my_macro
//!     as my_macro;
//! ```
//!
//! The hash is the XXH3 hash of the annotated item's `TokenStream`, and is
//! included to prevent name conflicts in the macro namespace.
//!
//! If you do not specify a `pub(in path)` restriction, you instead get a
//! world-visible macro:
//!
//! ```
//! #[macro_export]
//! #[doc(hidden)]
//! macro_rules! macro_impl_279137529572831871236407390024221977230_my_macro {
//!     () => {};
//! }
//! pub use macro_impl_279137529572831871236407390024221977230_my_macro
//!     as my_macro;
//! ```
//!
//! # Documenting public macros
//!
//! Unfortunately, `#[doc(hidden)]` on the actual macro implementation hides
//! any documentation attatched to it, `#[doc(inline)]`ing the `use` juts makes
//! it hidden as well, and you can't attach documentation. Thus, on stable, your
//! macro will be included in the documentation just as a re-export:
//!
//! ```
//! # pub struct macro_impl_279137529572831871236407390024221977230_my_macro;
//! pub use macro_impl_279137529572831871236407390024221977230_my_macro
//!     as my_macro;
//! ```
//!
//! and `macro_impl_279137529572831871236407390024221977230_my_macro` will not
//! be documented.
//!
//! If you are on nightly, however, we can take advantage of nightly features
//! in order to document the macro. In order to document your crate on nightly,
//! `#[macro_pub]` requires `#![cfg_attr(doc, feature(decl_macro, rustc_attrs))]`
//! and instead emits
//!
//! ```
//! #[cfg(doc)]
//! #[rustc_macro_transparency = "semitransparent"]
//! pub macro my_macro {
//!     () => {},
//! }
//! #[cfg(not(doc))]
//! // the previous expansion
//! # struct S;
//! ```
//!
//! This uses the unstable "macros 2.0" to define a macro with the legacy
//! `macro_rules!` hygeine rules that obeys normal scoping rules and is
//! documented cleanly by rustdoc.
//!
//! `macro_pub` automatically sniffs the rustc you're using to compile and
//! determines if it can use decl_macro and rustc_attrs in this way. When these
//! features inevitably get changed, `macro_pub` will automatically fall back to
//! the stable solution. Additionally, if/when a direct solution to this problem
//! is stabilized (e.g. `pub macro_rules!`, which has been discussed to do
//! almost exactly what this crate does), `macro_pub` will be updated to take
//! advantage of that on compatible rustc versions.
//!
//! # Examples
//!
//! In a module with `pub(crate)` visibility:
//!
//! ```
//! #[macro_use]
//! extern crate macro_pub;
//! # fn main() {}
//!
//! mod test {
//!     #[macro_pub(crate)]
//!     macro_rules! m {
//!         () => {};
//!     }
//! }
//!
//! test::m!();
//! ```
//!
//! With `pub(self)` visibility, it can't be accessed outside the module:
//!
//! ```compile_fail
//! #[macro_use]
//! extern crate macro_pub;
//! # fn main() {}
//!
//! mod test {
//!     #[macro_pub(self)]
//!     macro_rules! m {
//!         () => {};
//!     }
//! }
//!
//! test::m!();
//! ```

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use xxhash_rust::xxh3::xxh3_128;

#[proc_macro_attribute]
pub fn macro_pub(attr: TokenStream, item: TokenStream) -> TokenStream {
    let has_simple_decl_macro = cfg!(has_simple_decl_macro);
    let hash = xxh3_128(item.to_string().as_bytes());
    let error_output = {
        let mut output = item.clone();
        output.extend(
            r#"compile_error! { "`#[macro_pub]` must be used on a `macro_rules!` macro" }"#
                .parse::<TokenStream>()
                .unwrap(),
        );
        output
    };

    let mut attrs = TokenStream::new();
    let mut tokens = item.into_iter();

    let macro_rules = loop {
        match tokens.next() {
            Some(TokenTree::Ident(ident)) if ident.to_string() == "macro_rules" => {
                break TokenTree::Ident(ident);
            }
            // #[attribute]
            Some(TokenTree::Punct(punct)) if punct.as_char() == '#' => match tokens.next() {
                Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket => {
                    attrs.extend([TokenTree::Punct(punct), TokenTree::Group(group)])
                }
                _ => return error_output,
            },
            _ => return error_output,
        }
    };

    let bang = match tokens.next() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '!' => TokenTree::Punct(punct),
        _ => return error_output,
    };

    let macro_name = match tokens.next() {
        Some(TokenTree::Ident(ident)) => ident,
        _ => return error_output,
    };

    let macro_arms = match tokens.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => group.stream(),
        _ => return error_output,
    };

    let (vis, need_macro_export) = if attr.is_empty() {
        (
            [TokenTree::Ident(Ident::new("pub", Span::call_site()))]
                .into_iter()
                .collect::<TokenStream>(),
            true,
        )
    } else {
        (
            [
                TokenTree::Ident(Ident::new("pub", Span::call_site())),
                TokenTree::Group(Group::new(Delimiter::Parenthesis, attr)),
            ]
            .into_iter()
            .collect(),
            false,
        )
    };

    let macro_rules_name = TokenTree::Ident(Ident::new(
        &format!("macro_impl_{hash}_{macro_name}"),
        macro_name.span(),
    ));

    let mut output = attrs.clone();

    if has_simple_decl_macro && need_macro_export {
        output.extend(
            r##"#[cfg(doc)] #[rustc_macro_transparency = "semitransparent"]"##
                .parse::<TokenStream>()
                .unwrap(),
        );
        output.extend(vis.clone());
        output.extend([
            TokenTree::Ident(Ident::new("macro", Span::mixed_site())),
            TokenTree::Ident(macro_name.clone()),
            TokenTree::Group(Group::new(
                Delimiter::Brace,
                macro_arms
                    .clone()
                    .into_iter()
                    .map(|tt| match tt {
                        TokenTree::Punct(punct) if punct.as_char() == ';' => {
                            TokenTree::Punct(Punct::new(',', punct.spacing()))
                        }
                        tt => tt,
                    })
                    .collect(),
            )),
        ]);
        output.extend(attrs);
        output.extend(r##"#[cfg(not(doc))]"##.parse::<TokenStream>().unwrap());
    }

    if need_macro_export {
        output.extend(
            "#[macro_export] #[doc(hidden)]"
                .parse::<TokenStream>()
                .unwrap(),
        );
    }
    output.extend([
        macro_rules,
        bang,
        if need_macro_export {
            macro_rules_name.clone()
        } else {
            TokenTree::Ident(macro_name.clone())
        },
        TokenTree::Group(Group::new(Delimiter::Brace, macro_arms)),
    ]);

    if has_simple_decl_macro && need_macro_export {
        output.extend(r##"#[cfg(not(doc))]"##.parse::<TokenStream>().unwrap());
    }

    output.extend(vis);
    output.extend([
        TokenTree::Ident(Ident::new("use", Span::mixed_site())),
        if need_macro_export {
            macro_rules_name
        } else {
            TokenTree::Ident(macro_name.clone())
        },
        TokenTree::Ident(Ident::new("as", Span::mixed_site())),
        TokenTree::Ident(macro_name),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    output.extend(tokens);

    output
}
