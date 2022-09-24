use tokio::runtime::Runtime;
use wasm_spirv::{wasp, BufferRingConfig, Config, WgpuBackend, WgpuBackendConfig};
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
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();
    let conf = WgpuBackendConfig {
        buffer_ring_config: BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 128,
            buffer_size: 128,
        },
    };
    return wasp::WgpuBackend::new(device, queue, conf);
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

async fn test_assert_malformed_or_invalid(span: Span, mut module: QuoteWat<'_>, message: &str) {
    let bytes = match module.encode() {
        Ok(bs) => bs,
        Err(_) => return, // Failure to encode is fine if malformed
    };

    let backend = get_backend().await;

    let engine = wasp::Engine::new(backend, Config::default());

    let module = wasp::Module::new(&engine, bytes);

    assert!(
        module.is_err(),
        "assert malformed/invalid failed: {} at {:?}",
        message,
        span
    );
}
