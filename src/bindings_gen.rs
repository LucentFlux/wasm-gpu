use crate::{
    DATA_BINDING_INDEX, DATA_BINDING_READ_ONLY, ELEMENT_BINDING_INDEX, ELEMENT_BINDING_READ_ONLY,
    FLAGS_BINDING_INDEX, FLAGS_BINDING_READ_ONLY, IMMUTABLE_GLOBAL_BINDING_INDEX,
    IMMUTABLE_GLOBAL_BINDING_READ_ONLY, INPUT_BINDING_INDEX, INPUT_BINDING_READ_ONLY,
    MEMORY_BINDING_INDEX, MEMORY_BINDING_READ_ONLY, MUTABLE_GLOBAL_BINDING_INDEX,
    MUTABLE_GLOBAL_BINDING_READ_ONLY, OUTPUT_BINDING_INDEX, OUTPUT_BINDING_READ_ONLY,
    STACK_BINDING_INDEX, STACK_BINDING_READ_ONLY, TABLE_BINDING_INDEX, TABLE_BINDING_READ_ONLY,
};

use super::assembled_module::{build, WorkingModule};

pub(crate) struct BindingHandles {
    pub(crate) memory: naga::Handle<naga::GlobalVariable>,
    pub(crate) mutable_globals: naga::Handle<naga::GlobalVariable>,
    pub(crate) immutable_globals: naga::Handle<naga::GlobalVariable>,
    pub(crate) tables: naga::Handle<naga::GlobalVariable>,
    pub(crate) data: naga::Handle<naga::GlobalVariable>,
    pub(crate) elements: naga::Handle<naga::GlobalVariable>,

    pub(crate) input: naga::Handle<naga::GlobalVariable>,
    pub(crate) output: naga::Handle<naga::GlobalVariable>,

    pub(crate) stack: naga::Handle<naga::GlobalVariable>,

    pub(crate) flags: naga::Handle<naga::GlobalVariable>,
}

fn access(read_only: bool) -> naga::StorageAccess {
    if !read_only {
        naga::StorageAccess::LOAD | naga::StorageAccess::STORE
    } else {
        naga::StorageAccess::LOAD
    }
}

impl BindingHandles {
    pub(crate) fn new(working: &mut WorkingModule) -> build::Result<Self> {
        let word_array_ty = working.std_objs.tys.wasm_i32_array_buffer.get(working)?;
        let flags_ty = working.std_objs.tys.wasm_flags_buffer.get(working)?;

        Ok(Self {
            memory: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_memory".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: access(MEMORY_BINDING_READ_ONLY),
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
            mutable_globals: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_mutable_globals".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: access(MUTABLE_GLOBAL_BINDING_READ_ONLY),
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: MUTABLE_GLOBAL_BINDING_INDEX,
                    }),
                    ty: word_array_ty,
                    init: None,
                },
                naga::Span::UNDEFINED,
            ),
            immutable_globals: working.module.global_variables.append(
                naga::GlobalVariable {
                    name: Some("wasm_immutable_globals".to_owned()),
                    space: naga::AddressSpace::Storage {
                        access: access(IMMUTABLE_GLOBAL_BINDING_READ_ONLY),
                    },
                    binding: Some(naga::ResourceBinding {
                        group: 0,
                        binding: IMMUTABLE_GLOBAL_BINDING_INDEX,
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
                        access: access(TABLE_BINDING_READ_ONLY),
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
                        access: access(DATA_BINDING_READ_ONLY),
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
                        access: access(ELEMENT_BINDING_READ_ONLY),
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
                        access: access(INPUT_BINDING_READ_ONLY),
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
                        access: access(OUTPUT_BINDING_READ_ONLY),
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
                        access: access(STACK_BINDING_READ_ONLY),
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
            ),
        })
    }
}
