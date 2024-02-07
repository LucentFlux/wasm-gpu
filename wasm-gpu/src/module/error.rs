use thiserror::Error;
use wasmparser::BinaryReaderError;

/// A WebAssembly translation error.
///
/// When a WebAssembly function can't be translated, one of these error codes will be returned
/// to describe the failure.
#[derive(Error, Debug)]
pub enum WasmError {
    /// The input WebAssembly code is invalid.
    ///
    /// This error code is used by a WebAssembly translator when it encounters invalid WebAssembly
    /// code. This should never happen for validated WebAssembly code.
    #[error("Invalid input WebAssembly code at offset {offset}: {message}")]
    InvalidWebAssembly {
        /// A string describing the validation error.
        message: String,
        /// The bytecode offset where the error occurred.
        offset: usize,
    },

    /// A feature used by the WebAssembly code is not supported by the embedding environment.
    ///
    /// Embedding environments may have their own limitations and feature restrictions.
    #[error("Unsupported feature: {0}")]
    Unsupported(String),
}

impl From<BinaryReaderError> for WasmError {
    fn from(br: BinaryReaderError) -> Self {
        Self::InvalidWebAssembly {
            message: br.message().to_owned(),
            offset: br.offset(),
        }
    }
}
