use loupe::MemoryUsage;
use std::sync::Arc;
use wasmer::wasmparser::{Validator, WasmFeatures};
use wasmer::{CompilerConfig, Features, LocalFunctionIndex, ModuleMiddleware, Target};
use wasmer_compiler::{
    Compilation, CompileError, CompileModuleInfo, Compiler, FunctionBodyData,
    ModuleTranslationState,
};
use wasmer_types::entity::PrimaryMap;

#[derive(Clone, MemoryUsage)]
pub struct SPIRVCompilerConfig {
    canonicalize_nans: bool,
    deterministic_only: bool,
    #[loupe(skip)]
    middleware: Vec<Arc<dyn ModuleMiddleware>>,
}

impl SPIRVCompilerConfig {
    pub fn new() -> Self {
        Self {
            canonicalize_nans: false,
            deterministic_only: false,
            middleware: Vec::new(),
        }
    }

    pub fn deterministic_only(&mut self, enable: bool) {
        self.deterministic_only = enable;
    }
}

impl CompilerConfig for SPIRVCompilerConfig {
    fn enable_pic(&mut self) {
        panic!("PIC is not supported for SPIR-V")
    }

    fn enable_verifier(&mut self) {
        panic!("IR verification is not supported for SPIR-V")
    }

    fn enable_nan_canonicalization(&mut self) {
        self.canonicalize_nans(true);
    }

    fn canonicalize_nans(&mut self, enable: bool) {
        self.canonicalize_nans = enable;
    }

    fn compiler(self: Box<Self>) -> Box<dyn wasmer_compiler::Compiler> {
        Box::new(SPIRVCompiler::new(self))
    }

    fn default_features_for_target(&self, _target: &Target) -> Features {
        Features {
            // Trues
            simd: true,
            // Falses
            threads: false,
            reference_types: false,
            bulk_memory: false,
            multi_value: false,
            tail_call: false,
            module_linking: false,
            multi_memory: false,
            memory64: false,
            exceptions: false,
            relaxed_simd: false,
            extended_const: false,
        }
    }

    fn push_middleware(&mut self, middleware: Arc<dyn ModuleMiddleware>) {
        self.middleware.push(middleware);
    }
}

#[derive(MemoryUsage)]
struct SPIRVCompiler {
    cfg: SPIRVCompilerConfig,
}

impl SPIRVCompiler {
    pub fn new(cfg: Box<SPIRVCompilerConfig>) -> Self {
        Self { cfg: *cfg }
    }
}

impl Compiler for SPIRVCompiler {
    fn validate_module<'data>(
        &self,
        features: &Features,
        data: &'data [u8],
    ) -> Result<(), CompileError> {
        let mut validator = Validator::new();

        let wasm_features = WasmFeatures {
            bulk_memory: features.bulk_memory,
            threads: features.threads,
            reference_types: features.reference_types,
            multi_value: features.multi_value,
            simd: features.simd,
            tail_call: features.tail_call,
            module_linking: features.module_linking,
            multi_memory: features.multi_memory,
            memory64: features.memory64,
            exceptions: features.exceptions,
            deterministic_only: self.cfg.deterministic_only,
            extended_const: features.extended_const,
            relaxed_simd: features.relaxed_simd,
            mutable_global: true,
            saturating_float_to_int: true,
            sign_extension: true,
        };
        validator.wasm_features(wasm_features);
        validator
            .validate_all(data)
            .map_err(|e| CompileError::Validate(format!("{}", e)))?;
        Ok(())
    }

    fn compile_module<'data, 'module>(
        &self,
        target: &Target,
        module: &'module CompileModuleInfo,
        module_translation: &ModuleTranslationState,
        function_body_inputs: PrimaryMap<LocalFunctionIndex, FunctionBodyData<'data>>,
    ) -> Result<Compilation, CompileError> {
        Err(CompileError::UnsupportedFeature(
            "compilation not supported".to_string(),
        ))
    }

    fn get_middlewares(&self) -> &[Arc<dyn ModuleMiddleware>] {
        self.cfg.middleware.as_slice()
    }
}
