use glob::glob;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::env;
use std::path::Path;
use syn::spanned::Spanned;
use syn::{parse2, ItemFn, LitStr};
use wast::{
    lexer::Lexer,
    parser::{parse, ParseBuffer},
    Wast, WastDirective,
};

pub fn impl_tests(attr: TokenStream, f: ItemFn) -> TokenStream {
    let glob_str: syn::LitStr = parse2(attr).expect("unable to parse glob");
    let glob_str = glob_str.value();

    let path = env::var("CARGO_MANIFEST_DIR").expect("unable to get base dir");
    let path = Path::new(&path).join(glob_str);

    let fn_name = f.sig.ident.clone();

    let mut tests = Vec::new();
    let test_files =
        glob(path.to_str().expect("failed to make glob")).expect("failed to read glob pattern");
    for entry in test_files {
        let entry = entry.expect("error reading file");

        let source = std::fs::read_to_string(entry.clone())
            .expect(&format!("could not read file {}", entry.to_str().unwrap()));
        let mut lexer = Lexer::new(&source);
        lexer.allow_confusing_unicode(true);
        let buffer = ParseBuffer::new_with_lexer(lexer).expect(&format!(
            "could not create parse buffer {}",
            entry.to_str().unwrap()
        ));
        let wast = parse::<Wast>(&buffer)
            .expect(&format!("could not parse WAST {}", entry.to_str().unwrap()));

        for kind in wast.directives {
            match kind {
                WastDirective::AssertMalformed { .. }
                | WastDirective::AssertInvalid { .. }
                | WastDirective::AssertTrap { .. }
                | WastDirective::AssertReturn { .. }
                | WastDirective::AssertExhaustion { .. }
                | WastDirective::AssertUnlinkable { .. }
                | WastDirective::AssertException { .. } => {
                    let span = kind.span();

                    let entry_name = entry
                        .file_name()
                        .expect("glob matched non-file")
                        .to_str()
                        .unwrap()
                        .to_string()
                        .split(".")
                        .next()
                        .unwrap()
                        .to_string();
                    let entry_name = entry_name.replace("-", "_");
                    let (test_line, test_col) = span.linecol_in(&source);
                    let test_name = if test_col == 0 {
                        format_ident!("{}_{}_line_{}", fn_name, entry_name, test_line.to_string())
                    } else {
                        format_ident!(
                            "{}_{}_line_{}_col_{}",
                            fn_name,
                            entry_name,
                            test_line.to_string(),
                            test_col.to_string()
                        )
                    };
                    let test_path_literal = LitStr::new(entry.to_str().unwrap(), f.span());
                    let i = span.offset();
                    tests.push(quote! {
                        #[test]
                        pub fn #test_name() {
                            #fn_name(#test_path_literal, #i)
                        }
                    });

                    if std::env::var("FULL_TESTS") != Ok("true".to_owned()) {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    assert!(!tests.is_empty(), "found no tests");

    return quote! {
        #(#tests)*

        #f
    };
}
