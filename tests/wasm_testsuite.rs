use tokio::runtime::Runtime;
use wasm_spirv::{wasp, BufferRingConfig, Config};
use wast::lexer::Lexer;
use wast::token::Span;
use wast::{
    parser::{parse, ParseBuffer},
    QuoteWat, Wast, WastDirective,
};

#[wasm_spirv_test_gen::wast("tests/testsuite/*.wast")]
fn gen_check(path: &str, test_index: usize) {
    check(path, test_index)
}

pub async fn get_backend() -> wasp::WgpuBackend {
    let conf = wasp::WgpuBackendConfig {
        buffer_ring: BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 2 * 1024,
        },
        ..Default::default()
    };
    return wasp::WgpuBackend::new(conf, None)
        .await
        .expect("failed to get backend");
}

#[inline(never)] // Reduce code bloat to avoid OOM sigkill
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
            Runtime::new().unwrap().block_on(run_test(kind));
            return;
        }
    }
}

async fn run_test(directive: WastDirective<'_>) {
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
        } => test_assert_malformed_or_invalid(span, module, message).await,
        WastDirective::AssertInvalid {
            span,
            module,
            message,
        } => test_assert_malformed_or_invalid(span, module, message).await,
        WastDirective::AssertTrap {
            span,
            exec,
            message,
        } => {
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

async fn test_assert_malformed_or_invalid(span: Span, mut module: QuoteWat<'_>, message: &str) {
    let bytes = match module.encode() {
        Ok(bs) => bs,
        Err(_) => return, // Failure to encode is fine if malformed
    };

    let backend = get_backend().await;

    let engine = wasp::Engine::new(backend, Config::default());

    let module = wasp::Module::new(&engine, &bytes, "test");

    assert!(
        module.is_err(),
        "assert malformed/invalid failed: {} at {:?}",
        message,
        span
    );
}

async fn test_assert_trap(span: Span, mut module: QuoteWat<'_>, message: &str) {
    let bytes = match module.encode() {
        Ok(bs) => bs,
        Err(_) => return, // Failure to encode is fine if malformed
    };

    let backend = get_backend().await;

    let engine = wasp::Engine::new(backend, Config::default());

    let module = wasp::Module::new(&engine, &bytes, "test");

    assert!(
        module.is_err(),
        "assert malformed/invalid failed: {} at {:?}",
        message,
        span
    );
}
