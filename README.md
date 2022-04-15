`#[macro_pub]` is a replacement for `#[macro_export]` which provides normal
visibility rules for `macro_rules!` macros.

If you want the macro to not have world-public visibility, then use
`pub(in path)` syntax in the attribute, e.g. `#[macro_pub(crate)]`.

# How

If you `use` a legacy scoped macro, it "mounts" it to the normal visibility
and can be used as a normal item from then on. So when you write

```rust
#[macro_pub(crate)]
macro_rules! my_macro {
    () => {};
}
```

it expands to something like

```rust
macro_rules! macro_impl_279137529572831871236407390024221977230_my_macro {
    () => {};
}
pub(crate) use macro_impl_279137529572831871236407390024221977230_my_macro
    as my_macro;
```

The hash is the XXH3 hash of the annotated item's `TokenStream`, and is
included to prevent name conflicts in the macro namespace.

If you do not specify a `pub(in path)` restriction, you instead get a
world-visible macro:

```rust
#[macro_export]
#[doc(hidden)]
macro_rules! macro_impl_279137529572831871236407390024221977230_my_macro {
    () => {};
}
pub use macro_impl_279137529572831871236407390024221977230_my_macro
    as my_macro;
```

# Documenting public macros

Unfortunately, `#[doc(hidden)]` on the actual macro implementation hides
any documentation attatched to it, `#[doc(inline)]`ing the `use` juts makes
it hidden as well, and you can't attach documentation. Thus, on stable, your
macro will be included in the documentation just as a re-export:

```rust
pub use macro_impl_279137529572831871236407390024221977230_my_macro
    as my_macro;
```

and `macro_impl_279137529572831871236407390024221977230_my_macro` will not
be documented.

If you are on nightly, however, we can take advantage of nightly features
in order to document the macro. In order to document your crate on nightly,
`#[macro_pub]` requires `#![cfg_attr(doc, feature(decl_macro, rustc_attrs))]`
and instead emits

```rust
#[cfg(doc)]
#[rustc_macro_transparency = "semitransparent"]
pub macro my_macro {
    () => {},
}
#[cfg(not(doc))]
// the previous expansion
```

This uses the unstable "macros 2.0" to define a macro with the legacy
`macro_rules!` hygeine rules that obeys normal scoping rules and is
documented cleanly by rustdoc.

`macro_pub` automatically sniffs the rustc you're using to compile and
determines if it can use decl_macro and rustc_attrs in this way. When these
features inevitably get changed, `macro_pub` will automatically fall back to
the stable solution. Additionally, if/when a direct solution to this problem
is stabilized (e.g. `pub macro_rules!`, which has been discussed to do
almost exactly what this crate does), `macro_pub` will be updated to take
advantage of that on compatible rustc versions.

# Examples

In a module with `pub(crate)` visibility:

```rust
#[macro_use]
extern crate macro_pub;

mod test {
    #[macro_pub(crate)]
    macro_rules! m {
        () => {};
    }
}

test::m!();
```

With `pub(self)` visibility, it can't be accessed outside the module:

```rust
#[macro_use]
extern crate macro_pub;
# fn main() {}

mod test {
    #[macro_pub(self)]
    macro_rules! m {
        () => {};
    }
}

test::m!(); //~ ERROR
```
