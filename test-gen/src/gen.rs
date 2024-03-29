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

    let asyncness = f.sig.asyncness;

    let mut tests = Vec::new();
    let test_files = glob(path.to_str().expect("failed to make glob"))
        .expect("failed to read glob pattern")
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    for entry in test_files {
        let file_name = entry
            .file_name()
            .expect("glob matched non-file")
            .to_str()
            .unwrap()
            .to_string();
        let entry_name = file_name.split(".").next().unwrap().to_string();

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

        let line_count = source.lines().count();
        let line_padding = f32::floor(f32::log10(line_count as f32)) as usize + 1;

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

                    let entry_name = entry_name.replace("-", "_");
                    let (test_line, test_col) = span.linecol_in(&source);
                    let test_line = test_line + 1;
                    let test_name = if test_col <= 1 {
                        format!(
                            "{}_{}_line_{:0line_padding$}",
                            fn_name,
                            entry_name,
                            test_line,
                            line_padding = line_padding
                        )
                    } else {
                        format!(
                            "{}_{}_line_{:0line_padding$}_col_{}",
                            fn_name,
                            entry_name,
                            test_line,
                            test_col,
                            line_padding = line_padding
                        )
                    };
                    let test_name = format_ident!("{}", test_name);
                    let test_path_literal = LitStr::new(entry.to_str().unwrap(), f.span());
                    let i = span.offset();

                    let test_attr = if asyncness.is_some() {
                        quote! { #[tokio::test] }
                    } else {
                        quote! { #[test] }
                    };

                    let test_call = if asyncness.is_some() {
                        quote! { #fn_name(#test_path_literal, #i).await }
                    } else {
                        quote! { #fn_name(#test_path_literal, #i) }
                    };

                    tests.push(quote! {
                        #test_attr
                        pub #asyncness fn #test_name() {
                            #test_call
                        }
                    });
                }
                WastDirective::Wat(_)
                | WastDirective::Register { .. }
                | WastDirective::Invoke(_) => {}
                WastDirective::Thread(_) | WastDirective::Wait { .. } => unimplemented!(),
            }
        }
    }

    assert!(!tests.is_empty(), "found no tests");

    return quote! {
        #(#tests)*

        #f
    };
}
