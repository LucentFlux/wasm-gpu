use crate::session::{
    DATA_BINDING_INDEX, ELEMENT_BINDING_INDEX, FLAGS_BINDING_INDEX, GLOBAL_BINDING_INDEX,
    INPUT_BINDING_INDEX, MEMORY_BINDING_INDEX, OUTPUT_BINDING_INDEX, STACK_BINDING_INDEX,
    TABLE_BINDING_INDEX,
};

use super::assembled_module::{build, WorkingModule};

pub struct BindingHandles {
    pub memory: naga::Handle<naga::GlobalVariable>,
    pub globals: naga::Handle<naga::GlobalVariable>,
    pub tables: naga::Handle<naga::GlobalVariable>,
    pub data: naga::Handle<naga::GlobalVariable>,
    pub elements: naga::Handle<naga::GlobalVariable>,

    pub input: naga::Handle<naga::GlobalVariable>,
    pub output: naga::Handle<naga::GlobalVariable>,

    pub stack: naga::Handle<naga::GlobalVariable>,

    pub flags: naga::Handle<naga::GlobalVariable>,
}

impl BindingHandles {
    pub fn new(working: &mut WorkingModule) -> build::Result<Self> {
        let word_array_ty = working.std_objs.tys.wasm_i32_array_buffer.get(working)?;
        let flags_ty = working.std_objs.tys.wasm_flags_buffer.get(working)?;

        Ok(Self {
            memory: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_memory".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: MEMORY_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            globals: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_globals".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: GLOBAL_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            tables: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_tables".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: TABLE_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            data: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_datas".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: DATA_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            elements: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_elements".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: ELEMENT_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),

            // I/O
            input: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_input".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: INPUT_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            output: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_output".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: OUTPUT_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),

            // Stack emulation
            stack: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_stack".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: STACK_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),

            // Flags used to mark execution states
            flags: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_exec_flags".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: FLAGS_BINDING_INDEX,
                    }),
                    ty: flags_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
        })
    }
}
