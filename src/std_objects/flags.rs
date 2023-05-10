use naga_ext::{naga_expr, BlockExt, ShaderPart};
use wasmtime_environ::Trap;

use crate::trap_to_u32;

fn make_trap_constant(
    module: &mut naga::Module,
    trap: Option<Trap>,
) -> naga::Handle<naga::Constant> {
    let trap_id = trap_to_u32(trap);

    let name = match trap {
        Some(trap) => format!("{:?}", trap).to_uppercase(),
        None => "UNSET".to_owned(),
    };

    module.constants.append(
        naga::Constant {
            name: Some(format!("TRAP_{:?}", name)),
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Uint(trap_id as u64),
            },
        },
        naga::Span::UNDEFINED,
    )
}

#[derive(Clone)]
pub(crate) struct TrapValuesInstance {
    no_trap: naga::Handle<naga::Constant>,
    stack_overflow: naga::Handle<naga::Constant>,
    memory_out_of_bounds: naga::Handle<naga::Constant>,
    heap_misaligned: naga::Handle<naga::Constant>,
    table_out_of_bounds: naga::Handle<naga::Constant>,
    indirect_call_to_null: naga::Handle<naga::Constant>,
    bad_signature: naga::Handle<naga::Constant>,
    integer_overflow: naga::Handle<naga::Constant>,
    integer_division_by_zero: naga::Handle<naga::Constant>,
    bad_conversion_to_integer: naga::Handle<naga::Constant>,
    unreachable_code_reached: naga::Handle<naga::Constant>,
    interrupt: naga::Handle<naga::Constant>,
    always_trap_adapter: naga::Handle<naga::Constant>,
    out_of_fuel: naga::Handle<naga::Constant>,
    atomic_wait_non_shared_memory: naga::Handle<naga::Constant>,
}

impl TrapValuesInstance {
    pub(super) fn gen(module: &mut naga::Module) -> Self {
        Self {
            no_trap: make_trap_constant(module, None),
            stack_overflow: make_trap_constant(module, Some(Trap::StackOverflow)),
            memory_out_of_bounds: make_trap_constant(module, Some(Trap::MemoryOutOfBounds)),
            heap_misaligned: make_trap_constant(module, Some(Trap::HeapMisaligned)),
            table_out_of_bounds: make_trap_constant(module, Some(Trap::TableOutOfBounds)),
            indirect_call_to_null: make_trap_constant(module, Some(Trap::IndirectCallToNull)),
            bad_signature: make_trap_constant(module, Some(Trap::BadSignature)),
            integer_overflow: make_trap_constant(module, Some(Trap::IntegerOverflow)),
            integer_division_by_zero: make_trap_constant(module, Some(Trap::IntegerDivisionByZero)),
            bad_conversion_to_integer: make_trap_constant(
                module,
                Some(Trap::BadConversionToInteger),
            ),
            unreachable_code_reached: make_trap_constant(
                module,
                Some(Trap::UnreachableCodeReached),
            ),
            interrupt: make_trap_constant(module, Some(Trap::Interrupt)),
            always_trap_adapter: make_trap_constant(module, Some(Trap::AlwaysTrapAdapter)),
            out_of_fuel: make_trap_constant(module, Some(Trap::OutOfFuel)),
            atomic_wait_non_shared_memory: make_trap_constant(
                module,
                Some(Trap::AtomicWaitNonSharedMemory),
            ),
        }
    }

    pub fn get(&self, trap: Trap) -> naga::Handle<naga::Constant> {
        match trap {
            Trap::StackOverflow => self.stack_overflow,
            Trap::MemoryOutOfBounds => self.memory_out_of_bounds,
            Trap::HeapMisaligned => self.heap_misaligned,
            Trap::TableOutOfBounds => self.table_out_of_bounds,
            Trap::IndirectCallToNull => self.indirect_call_to_null,
            Trap::BadSignature => self.bad_signature,
            Trap::IntegerOverflow => self.integer_overflow,
            Trap::IntegerDivisionByZero => self.integer_division_by_zero,
            Trap::BadConversionToInteger => self.bad_conversion_to_integer,
            Trap::UnreachableCodeReached => self.unreachable_code_reached,
            Trap::Interrupt => self.interrupt,
            Trap::AlwaysTrapAdapter => self.always_trap_adapter,
            Trap::OutOfFuel => self.out_of_fuel,
            Trap::AtomicWaitNonSharedMemory => self.atomic_wait_non_shared_memory,
            _ => unreachable!(),
        }
    }

    pub fn get_optional(&self, trap: Option<Trap>) -> naga::Handle<naga::Constant> {
        match trap {
            Some(trap) => self.get(trap),
            None => self.no_trap,
        }
    }

    /// Emits instructions to set the global trap state to the new trap, if it is unset
    pub fn emit_set_trap(
        &self,
        trap: Trap,
        trap_global: naga::Handle<naga::GlobalVariable>,
        active: &mut impl ShaderPart,
    ) {
        let new_trap_code = self.get(trap);
        let new_trap_code = naga_expr!(active => Constant(new_trap_code));

        let trap_code_ptr = naga_expr!(active => Global(trap_global));

        let trap_code_current_value = naga_expr!(active => Load(trap_code_ptr));
        let mut if_unset = naga::Block::default();
        if_unset.push_store(trap_code_ptr, new_trap_code);

        let unset = naga_expr!(active => Constant(self.no_trap));
        let is_unset = naga_expr!(active => trap_code_current_value == unset);
        active
            .parts()
            .2
            .push_if(is_unset, if_unset, naga::Block::default());
    }
}
