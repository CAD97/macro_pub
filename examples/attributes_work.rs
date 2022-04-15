#![cfg_attr(has_simple_decl_macro, cfg_attr(doc, feature(decl_macro, rustc_attrs)))]

#[macro_use]
extern crate macro_pub;

/// I'm a macro with documentation!
#[macro_pub]
macro_rules! m {
    () => {};
}

pub fn main() {}
