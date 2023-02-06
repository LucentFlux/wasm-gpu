use crate::{
    func::assembled_module::BuildError, module::operation::OperatorByProposal, wasm_ty_bytes, Val,
};

use super::{add_val_type, body_gen::FunctionBodyInformation, mvp::eat_mvp_operator};

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub struct BasicBlockState<'a> {
    // Global shader data, e.g. types or constants
    module: &'a mut naga::Module,

    // The function we're building a body for
    function_handle: naga::Handle<naga::Function>,

    // naga::Handles into the above module
    func_body_info: FunctionBodyInformation<'a>,

    // What we're building into to make the function body
    stack: Vec<naga::Handle<naga::Expression>>,
    statements: Vec<naga::Statement>,
}

impl<'a> BasicBlockState<'a> {
    fn get_func_mut(&mut self) -> &mut naga::Function {
        self.module.functions.get_mut(self.function_handle)
    }

    /// Pushes an expression on to the current stack
    pub fn push(&mut self, value: naga::Expression) {
        let handle = self
            .get_func_mut()
            .expressions
            .append(value, naga::Span::UNDEFINED);
        self.stack.push(handle);
    }

    /// Pops an expression from the current stack
    pub fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    fn new_local_from_val_type(
        &mut self,
        ty: wasmparser::ValType,
    ) -> naga::Handle<naga::LocalVariable> {
        let ty = add_val_type(ty, &mut self.module.types);
        self.new_local(naga::LocalVariable {
            ty,
            name: None,
            init: None,
        })
    }

    fn new_local(&mut self, local: naga::LocalVariable) -> naga::Handle<naga::LocalVariable> {
        let func = self.module.functions.get_mut(self.function_handle.clone());
        func.local_variables.append(local, naga::Span::UNDEFINED)
    }

    /// Inserts a new constant using a given value
    pub fn constant(&mut self, value: Val) -> naga::Handle<naga::Constant> {
        let width = wasm_ty_bytes(value.get_type());

        let constant = if let Val::V128(v) = value {
            // Decompose u128 into uvec4 of u32s
            let vec_component_width = width / 4;
            assert_eq!(vec_component_width, 4);
            let vec_base_ty = self.module.types.insert(
                naga::Type {
                    name: None,
                    inner: naga::TypeInner::Vector {
                        size: naga::VectorSize::Quad,
                        kind: naga::ScalarKind::Uint,
                        width: vec_component_width,
                    },
                },
                naga::Span::UNDEFINED,
            );
            naga::ConstantInner::Composite {
                ty: vec_base_ty,
                components: (0..4)
                    .map(|i_word| {
                        let word = v >> (8 * vec_component_width * i_word);
                        let word =
                            u32::try_from(word & 0xFFFFFFFF).expect("truncated word fits in u32");
                        self.module.constants.append(
                            naga::Constant {
                                name: None,
                                specialization: None,
                                inner: naga::ConstantInner::Scalar {
                                    width: vec_component_width,
                                    value: naga::ScalarValue::Uint(word.into()),
                                },
                            },
                            naga::Span::UNDEFINED,
                        )
                    })
                    .collect(),
            }
        } else {
            naga::ConstantInner::Scalar {
                width,
                value: match value {
                    Val::I32(v) => naga::ScalarValue::Sint(v.into()),
                    Val::I64(v) => naga::ScalarValue::Sint(v),
                    Val::F32(v) => naga::ScalarValue::Float(v.to_float().into()),
                    Val::F64(v) => naga::ScalarValue::Float(v.to_float()),
                    Val::FuncRef(v) => {
                        naga::ScalarValue::Uint(v.as_u32().unwrap_or(u32::MAX).into())
                    }
                    Val::ExternRef(v) => {
                        naga::ScalarValue::Uint(v.as_u32().unwrap_or(u32::MAX).into())
                    }
                    Val::V128(v) => unreachable!(),
                },
            }
        };

        self.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: constant,
            },
            naga::Span::UNDEFINED,
        )
    }
}

/// Populates until a control flow instruction
pub fn build_basic_block(
    mut stack: Vec<naga::Handle<naga::Expression>>,
    instructions: &mut impl Iterator<Item = OperatorByProposal>,
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    func_body_info: FunctionBodyInformation,
) -> Result<(Vec<naga::Handle<naga::Expression>>, naga::Block), BuildError> {
    let mut state = BasicBlockState {
        module,
        function_handle,
        func_body_info,
        stack,
        statements: Vec::new(),
    };

    let mut instructions = instructions.peekable();
    while let Some(operation) = instructions.peek() {
        let res = match operation {
            OperatorByProposal::ControlFlow(_) => break,
            OperatorByProposal::MVP(mvp_op) => eat_mvp_operator(&mut state, mvp_op)?,
            OperatorByProposal::Exceptions(_)
            | OperatorByProposal::TailCall(_)
            | OperatorByProposal::ReferenceTypes(_)
            | OperatorByProposal::SignExtension(_)
            | OperatorByProposal::SaturatingFloatToInt(_)
            | OperatorByProposal::BulkMemory(_)
            | OperatorByProposal::Threads(_)
            | OperatorByProposal::SIMD(_)
            | OperatorByProposal::RelaxedSIMD(_) => {
                return Err(BuildError::UnsupportedInstructionError {
                    instruction_opcode: operation.opcode(),
                })
            }
        };

        // If it wasn't control flow, actually progress iterator since we implemented the operation
        instructions.next();
    }

    return Ok((state.stack, naga::Block::from_vec(state.statements)));
}
