use wasmparser::for_each_operator;

macro_rules! define_opcode {
    ($(@$proposal:ident $op:ident $({ $($payload:tt)* })? => $visit:ident)*) => {
        /// Instruction opcodes as defined [here].
        ///
        /// [here]: https://webassembly.github.io/spec/core/binary/instructions.html
        #[derive(Debug, Copy, Clone)]
        #[allow(missing_docs)]
        pub enum OpCode {
            $(
                $op,
            )*
        }

        impl OpCode {
            pub fn from_operator<'a>(op: &wasmparser::Operator<'a>) -> Self {
                match op {
                    $(
                        wasmparser::Operator::$op{..} => Self::$op,
                    )*
                }
            }
        }
    }
}
for_each_operator!(define_opcode);

/// Defines a new macro that filters operations by a proposal
macro_rules! filter_operators {
    ($macro_name:ident (@ $filter_token:tt $(, !$filter_op:tt)* $(,)?) | $called_macro:ident($($args:tt)*)) => {
        macro_rules! $macro_name {
            ((munch) { $$($$filtered:tt)* }) => {
                $called_macro!{$($args)* $$($$filtered)*}
            };
            $(
                ((munch) { $$($$filtered:tt)* } @$$proposal:ident $filter_op $$({ $$($$payload:tt)* })? => $$visit:ident $$($$others:tt)*) => {
                    $macro_name!{(munch) { $$($$filtered)* } $$($$others)*}
                };
            )*
            ((munch) { $$($$filtered:tt)* } @$filter_token $$op:ident $$({ $$($$payload:tt)* })? => $$visit:ident $$($$others:tt)*) => {
                $macro_name!{(munch) { $$($$filtered)* @$filter_token $$op $$({ $$($payload)* })? => $$visit } $$($$others)*}
            };
            ((munch) { $$($$filtered:tt)* } @$$proposal:ident $$op:ident $$({ $$($$payload:tt)* })? => $$visit:ident $$($$others:tt)*) => {
                $macro_name!{(munch) { $$($$filtered)* } $$($$others)*}
            };
            ($$($$others:tt)*) => {
                $macro_name!{(munch) { } $$($$others)*}
            }
        }
    }
}

macro_rules! define_make_operator_fn {
    ($enum_name:ident( $struct_name:ident :: $op:ident $({ $($field:ident : $field_ty:ty),* $(,)? })? )) => {
        paste::paste!{
            fn [< make_op_by_proposal_ $op:snake >] <'a> ( $($($field : $field_ty),* )* ) -> Result<OperatorByProposal, wasmparser::BinaryReaderError> {
                Ok(OperatorByProposal::$enum_name(
                    $struct_name::$op {$($($field),* )*}
                ))
            }
        }
    }
}

/// Defines a struct with some identity, to be used with the filter to have a set of only some opcodes
macro_rules! define_proposal_operator {
    ($struct_name:ident, $enum_name:ident $(@$proposal:ident $op:ident $({ $($field:ident : $field_ty:ty),* $(,)? })? => $visit:ident)*) => {
        #[derive(Clone, Debug)]
        #[allow(missing_docs)]
        pub enum $struct_name {
            $(
                $op $({ $($field : $field_ty,)* })?,
            )*
        }

        impl $struct_name {
            pub fn opcode(&self) -> OpCode {
                match &self {
                    $(
                        Self::$op { .. } => OpCode::$op,
                    )*
                }
            }
        }

        $(
            define_make_operator_fn! { $enum_name ( $struct_name :: $op $({ $($field : $field_ty),* })* ) }
        )*
    }
}

#[derive(Clone, Debug)]
pub enum ControlFlowOperator {
    End,
    Block {
        blockty: wasmparser::BlockType,
    },
    Loop {
        blockty: wasmparser::BlockType,
    },
    If {
        blockty: wasmparser::BlockType,
    },
    Else,
    Br {
        relative_depth: u32,
    },
    BrIf {
        relative_depth: u32,
    },
    BrTable {
        targets: Vec<u32>,
        default_target: u32,
    },
    Return,
    Call {
        function_index: u32,
    },
    CallIndirect {
        type_index: u32,
        table_index: u32,
        table_byte: u8,
    },
}

impl ControlFlowOperator {
    pub fn opcode(&self) -> OpCode {
        match self {
            ControlFlowOperator::End => OpCode::End,
            ControlFlowOperator::Block { .. } => OpCode::Block,
            ControlFlowOperator::Loop { .. } => OpCode::Loop,
            ControlFlowOperator::If { .. } => OpCode::If,
            ControlFlowOperator::Else => OpCode::Else,
            ControlFlowOperator::Br { .. } => OpCode::Br,
            ControlFlowOperator::BrIf { .. } => OpCode::BrIf,
            ControlFlowOperator::BrTable { .. } => OpCode::BrTable,
            ControlFlowOperator::Return => OpCode::Return,
            ControlFlowOperator::Call { .. } => OpCode::Call,
            ControlFlowOperator::CallIndirect { .. } => OpCode::CallIndirect,
        }
    }
}

filter_operators!(filter_define_mvp(@mvp, 
    !End,
    !Block,
    !Loop,
    !If,
    !Else,
    !Br,
    !BrIf,
    !BrTable,
    !Return,
    !Call,
    !CallIndirect,
) | define_proposal_operator(MVPOperator, MVP));
for_each_operator!(filter_define_mvp);
filter_operators!(filter_define_exceptions(@exceptions) | define_proposal_operator(ExceptionsOperator, Exceptions));
for_each_operator!(filter_define_exceptions);
filter_operators!(filter_define_tail_call(@tail_call) | define_proposal_operator(TailCallOperator, TailCall));
for_each_operator!(filter_define_tail_call);
filter_operators!(filter_define_reference_types(@reference_types) | define_proposal_operator(ReferenceTypesOperator, ReferenceTypes));
for_each_operator!(filter_define_reference_types);
filter_operators!(filter_define_sign_extension(@sign_extension) | define_proposal_operator(SignExtensionOperator, SignExtension));
for_each_operator!(filter_define_sign_extension);
filter_operators!(filter_define_saturating_float_to_int(@saturating_float_to_int) | define_proposal_operator(SaturatingFloatToIntOperator, SaturatingFloatToInt));
for_each_operator!(filter_define_saturating_float_to_int);
filter_operators!(filter_define_bulk_memory(@bulk_memory) | define_proposal_operator(BulkMemoryOperator, BulkMemory));
for_each_operator!(filter_define_bulk_memory);
filter_operators!(filter_define_threads(@threads) | define_proposal_operator(ThreadsOperator, Threads));
for_each_operator!(filter_define_threads);
filter_operators!(filter_define_simd(@simd) | define_proposal_operator(SIMDOperator, SIMD));
for_each_operator!(filter_define_simd);
filter_operators!(filter_define_relaxed_simd(@relaxed_simd) | define_proposal_operator(RelaxedSIMDOperator, RelaxedSIMD));
for_each_operator!(filter_define_relaxed_simd);

/// Used both as a way to make code easier to read, as well as a wway to remove the data lifetime of operators
#[derive(Clone, Debug)]
pub enum OperatorByProposal {
    ControlFlow(ControlFlowOperator),
    MVP(MVPOperator),
    Exceptions(ExceptionsOperator),
    TailCall(TailCallOperator),
    ReferenceTypes(ReferenceTypesOperator),
    SignExtension(SignExtensionOperator),
    SaturatingFloatToInt(SaturatingFloatToIntOperator),
    BulkMemory(BulkMemoryOperator),
    Threads(ThreadsOperator),
    SIMD(SIMDOperator),
    RelaxedSIMD(RelaxedSIMDOperator),
}

impl OperatorByProposal {
    pub fn opcode(&self) -> OpCode {
        match self {
            Self::ControlFlow(op) => op.opcode(),
            Self::MVP(op) => op.opcode(),
            Self::Exceptions(op) => op.opcode(),
            Self::TailCall(op) => op.opcode(),
            Self::ReferenceTypes(op) => op.opcode(),
            Self::SignExtension(op) => op.opcode(),
            Self::SaturatingFloatToInt(op) => op.opcode(),
            Self::BulkMemory(op) => op.opcode(),
            Self::Threads(op) => op.opcode(),
            Self::SIMD(op) => op.opcode(),
            Self::RelaxedSIMD(op) => op.opcode(),
        }
    }
}

// Custom add control flow operations to make processing easier
define_make_operator_fn! {ControlFlow(ControlFlowOperator::End)}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Block {
    blockty: wasmparser::BlockType,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Loop {
    blockty: wasmparser::BlockType,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::If {
    blockty: wasmparser::BlockType,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Else)}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Br {
    relative_depth: u32,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::BrIf {
    relative_depth: u32,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Return)}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::Call {
    function_index: u32,
})}
define_make_operator_fn! {ControlFlow(ControlFlowOperator::CallIndirect {
    type_index: u32,
    table_index: u32,
    table_byte: u8,
})}
fn make_op_by_proposal_br_table<'a>(
    targets: wasmparser::BrTable<'a>,
) -> Result<OperatorByProposal, wasmparser::BinaryReaderError> {
    let default_target = targets.default();
    let targets: Vec<u32> = targets.targets().collect::<Result<_, _>>()?;
    Ok(OperatorByProposal::ControlFlow(
        ControlFlowOperator::BrTable {
            default_target,
            targets,
        },
    ))
}

macro_rules! impl_op_by_proposal {
    ($(@$proposal:ident $op:ident $({ $($field:ident : $field_ty:ty),* $(,)? })? => $visit:ident)*) => {
        impl OperatorByProposal {
            pub fn from_operator<'a>(op: wasmparser::Operator<'a>) -> Result<Self, wasmparser::BinaryReaderError> {
                match op {
                    $(
                        wasmparser::Operator::$op $({ $($field),* })* => paste::paste!{ [< make_op_by_proposal_ $op:snake >] ( $($($field),* )* ) },
                    )*
                }
            }
        }
    }
}
for_each_operator!(impl_op_by_proposal);
