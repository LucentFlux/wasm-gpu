#![feature(iter_array_chunks)]

use itertools::Itertools;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;
use wasm_gpu::NamedExtern;
use wasm_gpu::{MappedStoreSetBuilder, ModuleInstanceReferences, Tuneables, WasmFeatures};
use wasm_types::{Val, V128};
use wast::core::{HeapType, NanPattern, V128Pattern, WastRetCore};
use wast::lexer::Lexer;
use wast::token::{Float32, Float64, Id, Index, Span};
use wast::{
    parser::{parse, ParseBuffer},
    QuoteWat, Wast, WastDirective, WastExecute, WastInvoke, WastRet, Wat,
};
use wgpu_async::{wrap_to_async, AsyncQueue};
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

//#[wasm_gpu_test_gen::wast("tests/testsuite/*.wast")]
#[wasm_gpu_test_gen::wast("tests/testsuite/i32.wast")]
#[wasm_gpu_test_gen::wast("tests/testsuite/f32.wast")]
//#[wasm_gpu_test_gen::wast("tests/testsuite/float_exprs.wast")]
async fn gen_check(path: &str, test_index: usize) {
    check(path, test_index).await
}

pub async fn get_backend() -> (MemorySystem, AsyncQueue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: adapter.limits(),
                label: None,
            },
            None,
        )
        .await
        .unwrap();

    let (device, queue) = wrap_to_async(device, queue);

    let memory_system = MemorySystem::new(
        &device,
        // Low memory footprint
        BufferRingConfig {
            chunk_size: 1024,
            total_transfer_buffers: 2,
        },
    )
    .unwrap();

    return (memory_system, queue);
}

struct WgpuState {
    memory_system: MemorySystem,
    queue: AsyncQueue,
}

static GPU_STATE: OnceCell<WgpuState> = OnceCell::new();
fn gpu<'a>() -> &'a WgpuState {
    GPU_STATE.get_or_init(WgpuState::new)
}

impl WgpuState {
    fn new() -> Self {
        let (memory_system, queue) = pollster::block_on(get_backend());
        Self {
            memory_system,
            queue,
        }
    }
}

struct WastState {
    features: WasmFeatures,
    store_builder: Option<MappedStoreSetBuilder>, // Taken when invoking
    named_modules: HashMap<String, Arc<ModuleInstanceReferences>>,
    latest_module: Option<Arc<ModuleInstanceReferences>>,
    imports: Vec<NamedExtern>,
}

const INSTANCE_COUNT: usize = 8;

impl WastState {
    async fn new() -> Self {
        Self {
            store_builder: Some(MappedStoreSetBuilder::new(
                &gpu().memory_system,
                "testsuite_storeset",
                Tuneables::default(),
            )),
            named_modules: HashMap::new(),
            latest_module: None,
            imports: Vec::new(),
            features: WasmFeatures {
                mutable_global: true,
                saturating_float_to_int: true,
                sign_extension: true,
                reference_types: true,
                multi_value: true,
                bulk_memory: true,
                simd: true,
                relaxed_simd: true,
                threads: true,
                tail_call: true,
                multi_memory: true,
                exceptions: true,
                memory64: true,
                extended_const: true,
                component_model: true,
                floats: true,
                function_references: true,
                memory_control: true,
            },
        }
    }

    async fn add_module<'a>(&'a mut self, mut quote_wast: QuoteWat<'a>, span: &Span, name: String) {
        let bytes = quote_wast
            .encode()
            .expect(&format!("could not encode expected module at {:?}", span));
        let module = wasm_gpu::Module::new(&self.features, &bytes, name)
            .expect(&format!("could not parse module byes at {:?}", span));
        let instance = self
            .store_builder
            .as_mut()
            .unwrap()
            .instantiate_module(&gpu().queue, &module, self.imports.clone())
            .await
            .expect(&format!("could not instantiate module at {:?}", span));

        let instance = Arc::new(instance);

        let id = match quote_wast {
            QuoteWat::Wat(Wat::Module(m)) => m.id,
            QuoteWat::QuoteModule(_, _) => unimplemented!("I don't know what this is"),
            QuoteWat::Wat(Wat::Component(_)) | QuoteWat::QuoteComponent(_, _) => {
                panic!("component model not supported")
            }
        };

        if let Some(id) = id {
            self.named_modules
                .insert(id.name().to_string(), instance.clone());
        }

        self.latest_module = Some(instance);
    }

    async fn register_module<'a>(&'a mut self, module: Option<Id<'a>>, name: &'a str, span: &Span) {
        let module = match module {
            None => self
                .latest_module
                .as_ref()
                .expect(&format!(
                    "register without module id, with no previous module at {:?}",
                    span
                ))
                .clone(),
            Some(id) => self
                .named_modules
                .get(id.name())
                .expect(&format!("no module with id {:?} at {:?}", id, span))
                .clone(),
        };
        let mut named_exports = module.get_named_exports(name);
        self.imports.append(&mut named_exports)
    }

    async fn invoke<'a>(
        &'a mut self,
        wast_invoke: WastInvoke<'a>,
        span: &Span,
    ) -> Result<Vec<Val>, wasmtime_environ::Trap> {
        let module = match wast_invoke.module {
            None => self
                .latest_module
                .as_ref()
                .expect(&format!(
                    "invoke without module id, with no previous module at {:?}",
                    span
                ))
                .clone(),
            Some(id) => self
                .named_modules
                .get(id.name())
                .expect(&format!("no module with id {:?} at {:?}", id, span))
                .clone(),
        };

        let func = module.get_func(wast_invoke.name).expect(&format!(
            "no function with name {} found at {:?}",
            wast_invoke.name, wast_invoke.span
        ));

        // Build
        let completed = self
            .store_builder
            .take()
            .unwrap()
            .complete(&gpu().queue)
            .await
            .unwrap();
        let mut instances = completed
            .build(&gpu().memory_system, &gpu().queue, INSTANCE_COUNT)
            .await
            .unwrap();

        // Invoke
        let args: Vec<Val> = wast_invoke
            .args
            .into_iter()
            .map(|v| Val::try_from(v).expect("args should be values"))
            .collect();
        let args: Vec<Vec<Val>> = (0..INSTANCE_COUNT).map(|_| args.clone()).collect();
        let mut res_list: Vec<Result<Vec<Val>, wasmtime_environ::Trap>> = func
            .call_all(&gpu().memory_system, &gpu().queue, &mut instances, args)
            .await
            .unwrap()
            .await
            .unwrap();

        // Many instances but should all be the same result
        let res = res_list.pop().unwrap();
        assert!(
            res_list
                .into_iter()
                .all(|other_res: Result<Vec<Val>, wasmtime_environ::Trap>| {
                    match (&res, &other_res) {
                        (Ok(v1), Ok(v2)) => v1.into_iter().zip(v2).all(|(a, b)| a.bitwise_eq(b)),
                        (Err(trap1), Err(trap2)) => trap1.eq(trap2),
                        _ => false,
                    }
                }),
            "all results were not the same"
        );

        // Unbuild
        self.store_builder = Some(
            instances
                .snapshot(&gpu().memory_system, &gpu().queue, 0)
                .await
                .unwrap(),
        );

        return res;
    }

    async fn exec<'a>(
        &'a mut self,
        exec: WastExecute<'a>,
        span: &Span,
    ) -> Result<Vec<Val>, wasmtime_environ::Trap> {
        match exec {
            WastExecute::Invoke(inv) => self.invoke(inv, span).await,
            WastExecute::Wat(_) => unimplemented!(),
            WastExecute::Get { .. } => unimplemented!(),
        }
    }
}

#[inline(never)] // Reduce code bloat to avoid OOM sigkill
async fn check(path: &str, test_offset: usize) {
    let source = std::fs::read_to_string(path).unwrap();
    let mut lexer = Lexer::new(&source);
    lexer.allow_confusing_unicode(true);
    let buffer = ParseBuffer::new_with_lexer(lexer)
        .expect(&format!("could not create parse buffer {}", path));
    let wast = parse::<Wast>(&buffer).unwrap();

    let mut state = WastState::new().await;

    // Parsed things
    for kind in wast.directives {
        let span = kind.span();
        match kind {
            WastDirective::Wat(quote_wast) => {
                state
                    .add_module(quote_wast, &span, format!("module_{}", span.offset()))
                    .await
            }
            WastDirective::Register {
                span: _,
                name,
                module,
            } => state.register_module(module, name, &span).await,
            WastDirective::Invoke(wast_invoke) => {
                state
                    .invoke(wast_invoke, &span)
                    .await
                    .expect(&format!("failed to run invoke at {:?}", span));
            }
            other_kind if (other_kind.span().offset() == test_offset) => {
                run_assertion(other_kind, state).await;
                return;
            }
            _ => {}
        }
    }
}

async fn run_assertion(directive: WastDirective<'_>, state: WastState) {
    match directive {
        WastDirective::Wat(_) | WastDirective::Register { .. } | WastDirective::Invoke(_) => {
            panic!("cannot test non-assert")
        }
        WastDirective::AssertMalformed {
            span,
            module,
            message,
        } => test_assert_malformed_or_invalid(state, span, module, message).await,
        WastDirective::AssertInvalid {
            span,
            module,
            message,
        } => test_assert_malformed_or_invalid(state, span, module, message).await,
        WastDirective::AssertTrap {
            span,
            exec,
            message,
        } => test_assert_trap(state, span, exec, message).await,
        WastDirective::AssertReturn {
            span,
            exec,
            results,
        } => test_assert_return(state, span, exec, results).await,
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

async fn test_assert_malformed_or_invalid(
    state: WastState,
    span: Span,
    mut module: QuoteWat<'_>,
    message: &str,
) {
    let bytes = match module.encode() {
        Ok(bs) => bs,
        Err(_) => return, // Failure to encode is fine if malformed
    };

    let module = wasm_gpu::Module::new(&state.features, &bytes, "test_module".to_owned());

    let module = match module {
        Err(_) => return, // We want this to fail
        Ok(module) => module,
    };

    let res = state
        .store_builder
        .unwrap()
        .instantiate_module(&gpu().queue, &module, state.imports.clone())
        .await;

    assert!(
        res.is_err(),
        "assert malformed/invalid failed: {} at {:?}",
        message,
        span
    );
}

async fn test_assert_trap<'a>(
    mut state: WastState,
    span: Span,
    exec: WastExecute<'a>,
    _message: &'a str,
) {
    let ret = state.exec(exec, &span).await;

    assert!(ret.is_err())
}

fn f32_matches(got: f32, expected: &NanPattern<Float32>) -> bool {
    match expected {
        NanPattern::CanonicalNan | NanPattern::ArithmeticNan => got.is_nan(),
        NanPattern::Value(v) => v.bits == got.to_bits(),
    }
}

fn f64_matches(got: f64, expected: &NanPattern<Float64>) -> bool {
    match expected {
        NanPattern::CanonicalNan | NanPattern::ArithmeticNan => got.is_nan(),
        NanPattern::Value(v) => v.bits == got.to_bits(),
    }
}

macro_rules! to_bytes {
    ($v:ident) => {
        $v.into_iter()
            .flat_map(|i| (*i).to_le_bytes())
            .collect_vec()
    };
}

fn vec_matches(got: V128, expected: &V128Pattern) -> bool {
    let bs = got.to_le_bytes();
    match expected {
        V128Pattern::I8x16(v) => to_bytes!(v) == bs,
        V128Pattern::I16x8(v) => to_bytes!(v) == bs,
        V128Pattern::I32x4(v) => to_bytes!(v) == bs,
        V128Pattern::I64x2(v) => to_bytes!(v) == bs,
        V128Pattern::F32x4(v) => bs
            .into_iter()
            .array_chunks::<4>()
            .into_iter()
            .map(|i| f32::from_le_bytes(i))
            .zip(v)
            .all(|(got, expected)| f32_matches(got, expected)),
        V128Pattern::F64x2(v) => bs
            .into_iter()
            .array_chunks::<8>()
            .into_iter()
            .map(|i| f64::from_le_bytes(i))
            .zip(v)
            .all(|(got, expected)| f64_matches(got, expected)),
    }
}

fn test_match(got: Val, expected: &WastRetCore) -> bool {
    match (expected, got) {
        (WastRetCore::I32(i1), Val::I32(i2)) => (*i1) == i2,
        (WastRetCore::I64(i1), Val::I64(i2)) => (*i1) == i2,
        (WastRetCore::F32(f1), Val::F32(f2)) => f32_matches(f2, f1),
        (WastRetCore::F64(f1), Val::F64(f2)) => f64_matches(f2, f1),
        (WastRetCore::V128(v1), Val::V128(v2)) => vec_matches(v2, v1),
        (WastRetCore::RefNull(Some(HeapType::Func)), Val::FuncRef(r)) => r.is_null(),
        (WastRetCore::RefNull(Some(HeapType::Extern)), Val::ExternRef(r)) => r.is_null(),
        (WastRetCore::RefFunc(None), Val::FuncRef(_)) => true,
        (WastRetCore::RefFunc(Some(Index::Num(v1, _))), Val::FuncRef(v2)) => {
            v2.as_u32() == Some(*v1)
        }
        (WastRetCore::RefFunc(Some(Index::Id(v1))), Val::FuncRef(v2)) => unimplemented!(),
        (WastRetCore::RefExtern(v1), Val::ExternRef(v2)) => v2.as_u32() == Some(*v1),
        (WastRetCore::Either(choices), got) => {
            choices.into_iter().any(|option| test_match(got, option))
        }
        _ => false,
    }
}

async fn test_assert_return<'a>(
    mut state: WastState,
    span: Span,
    mut exec: WastExecute<'a>,
    results: Vec<WastRet<'a>>,
) {
    let ret = state
        .exec(exec, &span)
        .await
        .expect("failed to run test assert return");

    if ret.len() != results.len() {
        panic!(
            "failed assert return: expected {:?} but got {:?}",
            results, ret
        )
    }

    for (expected, result) in results.iter().zip(ret.clone()) {
        let expected = match expected {
            WastRet::Core(c) => c,
            WastRet::Component(_) => panic!("component tests aren't supported"),
        };
        if !test_match(result, expected) {
            panic!(
                "failed assert return: expected {:?} but got {:?}",
                expected, result
            )
        }
    }
}
