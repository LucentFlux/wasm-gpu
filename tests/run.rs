use wast::{
    parser::{parse, ParseBuffer},
    Wast, WastDirective,
};

#[wasm_spirv_test_gen::wast("tests/testsuite/*.wast")]
pub fn check(path: &str, test_index: u32) {
    let source = std::fs::read_to_string(path).unwrap();
    let buffer = ParseBuffer::new(&source).unwrap();
    let wast = parse::<Wast>(&buffer).unwrap();

    // Parsed things

    let mut i = 0;
    for kind in wast.directives {
        match &kind {
            WastDirective::Wat(quote_wast) => {}
            WastDirective::Register { span, name, module } => {}
            WastDirective::Invoke(wast_invoke) => {}
            _ => {}
        }
        if i == test_index {
            run_test(kind);
            return;
        }
        i += 1;
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
        } => {
            panic!("assertion not implemented")
        }
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
