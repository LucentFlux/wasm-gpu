use wasm_spirv::compiler::SPIRVCompilerConfig;
use wasmer::{Module, Store, Universal};
use wast::lexer::Lexer;
use wast::token::Span;
use wast::{
    parser::{parse, ParseBuffer},
    QuoteWat, Wast, WastDirective,
};

#[wasm_spirv_test_gen::wast("tests/testsuite/*.wast")]
fn gen_check(path: &str, test_index: usize) {
    check(path, test_index);
}

fn check(path: &str, test_offset: usize) {
    let source = std::fs::read_to_string(path).unwrap();
    let mut lexer = Lexer::new(&source);
    lexer.allow_confusing_unicode(true);
    let buffer = ParseBuffer::new_with_lexer(lexer)
        .expect(&format!("could not create parse buffer {}", path));
    let wast = parse::<Wast>(&buffer).unwrap();

    // Parsed things
    for kind in wast.directives {
        match &kind {
            WastDirective::Wat(quote_wast) => {}
            WastDirective::Register { span, name, module } => {}
            WastDirective::Invoke(wast_invoke) => {}
            _ => {}
        }
        if kind.span().offset() == test_offset {
            run_test(kind);
            return;
        }
    }
}

fn run_test(directive: WastDirective) {
    match directive {
        WastDirective::Wat(_) => {
            panic!("wat not assertion")
        }
        WastDirective::Register { .. } => {
            panic!("register not assertion")
        }
        WastDirective::Invoke(_) => {
            panic!("invoke not assertion")
        }
        WastDirective::AssertMalformed {
            span,
            module,
            message,
        } => test_assert_malformed(span, module, message),
        WastDirective::AssertInvalid { .. } => {
            panic!("assertion not implemented")
        }
        WastDirective::AssertTrap { .. } => {
            panic!("assertion not implemented")
        }
        WastDirective::AssertReturn { .. } => {
            panic!("assertion not implemented")
        }
        WastDirective::AssertExhaustion { .. } => {
            panic!("assertion not implemented")
        }
        WastDirective::AssertUnlinkable { .. } => {
            panic!("assertion not implemented")
        }
        WastDirective::AssertException { .. } => {
            panic!("assertion not implemented")
        }
    }
}

fn test_assert_malformed(span: Span, mut module: QuoteWat, message: &str) {
    let bytes = match module.encode() {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut store = Store::new(&Universal::new(SPIRVCompilerConfig::new()).engine());

    assert!(
        Module::new(&store, bytes).is_err(),
        "assert malformed failed: {} at {:?}",
        message,
        span
    );
}
