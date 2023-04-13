use crate::build;
use crate::std_objects::Generator;
use crate::std_objects::{GenerationParameters, StdObjectsGenerator};
use crate::{
    DATA_BINDING_INDEX, DATA_BINDING_READ_ONLY, ELEMENT_BINDING_INDEX, ELEMENT_BINDING_READ_ONLY,
    FLAGS_BINDING_INDEX, FLAGS_BINDING_READ_ONLY, IMMUTABLE_GLOBAL_BINDING_INDEX,
    IMMUTABLE_GLOBAL_BINDING_READ_ONLY, INPUT_BINDING_INDEX, INPUT_BINDING_READ_ONLY,
    MEMORY_BINDING_INDEX, MEMORY_BINDING_READ_ONLY, MUTABLE_GLOBAL_BINDING_INDEX,
    MUTABLE_GLOBAL_BINDING_READ_ONLY, OUTPUT_BINDING_INDEX, OUTPUT_BINDING_READ_ONLY,
    STACK_BINDING_INDEX, STACK_BINDING_READ_ONLY, TABLE_BINDING_INDEX, TABLE_BINDING_READ_ONLY,
};

use super::GlobalGen;

fn access(read_only: bool) -> naga::StorageAccess {
    if !read_only {
        naga::StorageAccess::LOAD | naga::StorageAccess::STORE
    } else {
        naga::StorageAccess::LOAD
    }
}

fn make_word_binding<Ps: GenerationParameters>(
    module: &mut naga::Module,
    others: &StdObjectsGenerator<Ps>,
    name: &str,
    read_only: bool,
    binding: u32,
) -> build::Result<naga::Handle<naga::GlobalVariable>> {
    let word_array_ty = others.word_array_buffer_ty.gen(module, others)?;
    Ok(module.global_variables.append(
        naga::GlobalVariable {
            name: Some(name.to_owned()),
            space: naga::AddressSpace::Storage {
                access: access(read_only),
            },
            binding: Some(naga::ResourceBinding { group: 0, binding }),
            ty: word_array_ty,
            init: None,
        },
        naga::Span::UNDEFINED,
    ))
}

pub(crate) struct MemoryBindingGen;
impl GlobalGen for MemoryBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_memory",
            MEMORY_BINDING_READ_ONLY,
            MEMORY_BINDING_INDEX,
        )
    }
}

pub(crate) struct MutableGlobalsBindingGen;
impl GlobalGen for MutableGlobalsBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_mutable_globals",
            MUTABLE_GLOBAL_BINDING_READ_ONLY,
            MUTABLE_GLOBAL_BINDING_INDEX,
        )
    }
}

pub(crate) struct ImmutableGlobalsBindingGen;
impl GlobalGen for ImmutableGlobalsBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_immutable_globals",
            IMMUTABLE_GLOBAL_BINDING_READ_ONLY,
            IMMUTABLE_GLOBAL_BINDING_INDEX,
        )
    }
}

pub(crate) struct TablesBindingGen;
impl GlobalGen for TablesBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_tables",
            TABLE_BINDING_READ_ONLY,
            TABLE_BINDING_INDEX,
        )
    }
}

pub(crate) struct DataBindingGen;
impl GlobalGen for DataBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_datas",
            DATA_BINDING_READ_ONLY,
            DATA_BINDING_INDEX,
        )
    }
}

pub(crate) struct ElementsBindingGen;
impl GlobalGen for ElementsBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_elements",
            ELEMENT_BINDING_READ_ONLY,
            ELEMENT_BINDING_INDEX,
        )
    }
}

pub(crate) struct InputBindingGen;
impl GlobalGen for InputBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_input",
            INPUT_BINDING_READ_ONLY,
            INPUT_BINDING_INDEX,
        )
    }
}

pub(crate) struct OutputBindingGen;
impl GlobalGen for OutputBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_output",
            OUTPUT_BINDING_READ_ONLY,
            OUTPUT_BINDING_INDEX,
        )
    }
}

pub(crate) struct StackBindingGen;
impl GlobalGen for StackBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        make_word_binding(
            module,
            others,
            "wasm_stack",
            STACK_BINDING_READ_ONLY,
            STACK_BINDING_INDEX,
        )
    }
}

pub(crate) struct FlagsBindingGen;
impl GlobalGen for FlagsBindingGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        let flags_ty = others.flags_buffer_ty.gen(module, others)?;

        Ok(module.global_variables.append(
            naga::GlobalVariable {
                name: Some("wasm_exec_flags".to_owned()),
                space: naga::AddressSpace::Storage {
                    access: access(FLAGS_BINDING_READ_ONLY),
                },
                binding: Some(naga::ResourceBinding {
                    group: 0,
                    binding: FLAGS_BINDING_INDEX,
                }),
                ty: flags_ty,
                init: None,
            },
            naga::Span::UNDEFINED,
        ))
    }
}
