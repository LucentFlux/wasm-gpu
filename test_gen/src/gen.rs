use proc_macro2::{Punct, TokenStream, Ident};
use syn::{ItemFn, LitStr, parse2};
use syn::parse::{Parse, ParseStream};
use syn::ext::IdentExt;
use std::env;
use std::path::Path;
use glob::glob;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use wast::{parser::{parse, ParseBuffer}, lexer::Lexer, Wast, WastDirective};

pub fn impl_tests(attr: TokenStream, f: ItemFn) -> TokenStream {
    let glob_str: syn::LitStr = parse2(attr).expect("unable to parse glob");
    let glob_str = glob_str.value();

    let path = env::var("CARGO_MANIFEST_DIR").expect("unable to get base dir");
    let path = Path::new(&path).join(glob_str);

    let fn_name = f.sig.ident.clone();

    let mut tests = Vec::new();
    for entry in glob(path.to_str().expect("failed to make glob")).expect("failed to read glob pattern") {
        let entry = entry.expect("error reading file");

        let source = std::fs::read_to_string(entry.clone()).expect(&format!("could not read file {}", entry.to_str().unwrap()));
        let mut lexer = Lexer::new(&source);
        lexer.allow_confusing_unicode(true);
        let buffer = ParseBuffer::new_with_lexer(lexer).expect(&format!("could not create parse buffer {}", entry.to_str().unwrap()));
        let wast = parse::<Wast>(&buffer).expect(&format!("could not parse WAST {}", entry.to_str().unwrap()));

        let mut i = 0u32;
        for kind in wast.directives {
            match kind {
                WastDirective::AssertMalformed { .. }
                | WastDirective::AssertInvalid { .. }
                | WastDirective::AssertTrap { .. }
                | WastDirective::AssertReturn { .. }
                | WastDirective::AssertExhaustion { .. }
                | WastDirective::AssertUnlinkable { .. }
                | WastDirective::AssertException { .. } => {
                    let entry_name = entry.file_name()
                        .expect("glob matched non-file")
                        .to_str().unwrap()
                        .to_string()
                        .split(".")
                        .next().unwrap().to_string();
                    let entry_name = entry_name.replace("-", "_");
                    let test_name = format_ident!("{}_{}_{}", fn_name, entry_name, i);
                    let test_path_literal = LitStr::new(entry.to_str().unwrap(), f.span());
                    tests.push(quote! {
                        #[test]
                        pub fn #test_name() {
                            #fn_name(#test_path_literal, #i)
                        }
                    });
                }
                _ => {}
            }
            i += 1;
        }
    }

    return quote! {
        #(#tests)*

        #f
    }
}