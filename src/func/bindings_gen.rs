use crate::session::{
    DATA_BINDING_INDEX, ELEMENT_BINDING_INDEX, FLAGS_BINDING_INDEX, GLOBAL_BINDING_INDEX,
    INPUT_BINDING_INDEX, MEMORY_BINDING_INDEX, OUTPUT_BINDING_INDEX, STACK_BINDING_INDEX,
    TABLE_BINDING_INDEX, TRAP_FLAG_INDEX,
};

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
    pub fn new(module: &mut naga::Module) -> Self {
        let word_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Scalar {
                    kind: naga::ScalarKind::Uint,
                    width: 4,
                },
            },
            naga::Span::UNDEFINED,
        );
        let word_array_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: word_ty,
                    size: naga::ArraySize::Dynamic,
                    stride: 1,
                },
            },
            naga::Span::UNDEFINED,
        );

        let flag_members = vec![naga::StructMember {
            name: Some("trap_flag".to_owned()),
            ty: word_ty,
            binding: None,
            offset: TRAP_FLAG_INDEX,
        }];
        let flags_ty = module.types.insert(
            naga::Type {
                name: Some("wasm_flags".to_owned()),
                inner: naga::TypeInner::Struct {
                    span: u32::try_from(flag_members.len() * 4).expect("static size"),
                    members: flag_members,
                },
            },
            naga::Span::UNDEFINED,
        );

        Self {
            memory: module.global_variables.append(
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
            globals: module.global_variables.append(
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
            tables: module.global_variables.append(
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
            data: module.global_variables.append(
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
            elements: module.global_variables.append(
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
            input: module.global_variables.append(
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
            output: module.global_variables.append(
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
            stack: module.global_variables.append(
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
            flags: module.global_variables.append(
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
        }
    }
}
