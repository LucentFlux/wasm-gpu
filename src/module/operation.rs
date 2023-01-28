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
    ($macro_name:ident (@ $filter_token:tt $(, !$filter_op:tt)*) | $called_macro:ident($($args:tt)*)) => {
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

/// Defines a struct with some identity, to be used with the filter to have a set of only some opcodes
macro_rules! define_proposal_operator {
    ($struct_name:ident $({ $($predef_op:ident { $($predef_field:ident : $predef_field_ty:ty),* $(,)? }),* $(,)? })?, $enum_name:ident $(@$proposal:ident $op:ident $({ $($field:ident : $field_ty:ty),* $(,)? })? => $visit:ident)*) => {
        #[derive(Clone, Debug)]
        #[allow(missing_docs)]
        pub enum $struct_name {
            $(
                $(
                    $predef_op { $($predef_field : $predef_field_ty),* },
                )*
            )*
            $(
                $op $({ $($field : $field_ty,)* })?,
            )*
        }

        impl $struct_name {
            pub fn opcode(&self) -> OpCode {
                match &self {
                    $(
                        $(
                            Self::$predef_op { .. } => OpCode::$predef_op,
                        )*
                    )*
                    $(
                        Self::$op { .. } => OpCode::$op,
                    )*
                }
            }
        }

        $(
            paste::paste!{
                fn [< make_op_by_proposal_ $op:snake >] <'a> ( $($($field : $field_ty),* )* ) -> Result<OperatorByProposal, wasmparser::BinaryReaderError> {
                    Ok(OperatorByProposal::$enum_name(
                        $struct_name::$op {$($($field),* )*}
                    ))
                }
            }
        )*
    }
}
filter_operators!(filter_define_mvp(@mvp, !BrTable) | define_proposal_operator(MVPOperator{BrTable{default_target: u32, targets: Vec<u32>}}, MVP));
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

// Custom add operations that require more parsing (which is dumb)
fn make_op_by_proposal_br_table<'a>(
    targets: wasmparser::BrTable<'a>,
) -> Result<OperatorByProposal, wasmparser::BinaryReaderError> {
    let default_target = targets.default();
    let targets: Vec<u32> = targets.targets().collect::<Result<_, _>>()?;
    Ok(OperatorByProposal::MVP(MVPOperator::BrTable {
        default_target,
        targets,
    }))
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
