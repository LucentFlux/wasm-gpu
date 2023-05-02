use crate::build;

use super::std_objects_gen;

fn access(read_only: bool) -> naga::StorageAccess {
    if !read_only {
        naga::StorageAccess::LOAD | naga::StorageAccess::STORE
    } else {
        naga::StorageAccess::LOAD
    }
}

fn make_word_binding(
    module: &mut naga::Module,
    word_array_ty: std_objects_gen::WordArrayBufferTy,
    name: &str,
    read_only: bool,
    binding: u32,
) -> build::Result<naga::Handle<naga::GlobalVariable>> {
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

macro_rules! word_bindings {
    (struct $gen_struct_name:ident { flags, constants, $($name:ident),* $(,)? }) => {
        paste::paste!{
            #[perfect_derive::perfect_derive(Copy, Clone)]
            pub(crate) struct $gen_struct_name {
                pub(crate) flags: naga::Handle<naga::GlobalVariable>,
                pub(crate) constants: naga::Handle<naga::GlobalVariable>,
                $(
                    pub(crate) $name: naga::Handle<naga::GlobalVariable>,
                )*
            }

            impl $gen_struct_name {
                pub(super) fn gen(
                    module: &mut naga::Module,
                    constants_ty: std_objects_gen::WordArrayBufferTy,
                    word_array_ty: std_objects_gen::WordArrayBufferTy,
                    flags_array_ty: std_objects_gen::FlagsArrayBufferTy,
                ) -> crate::build::Result<Self> {
                    let flags = module.global_variables.append(
                        naga::GlobalVariable {
                            name: Some("wasm_exec_flags".to_owned()),
                            space: naga::AddressSpace::Storage {
                                access: access(crate::FLAGS_BINDING_READ_ONLY),
                            },
                            binding: Some(naga::ResourceBinding {
                                group: 0,
                                binding: crate::FLAGS_BINDING_INDEX,
                            }),
                            ty: flags_array_ty,
                            init: None,
                        },
                        naga::Span::UNDEFINED,
                    );
                    let constants = module.global_variables.append(
                        naga::GlobalVariable {
                            name: Some("wasm_exec_constants".to_owned()),
                            space: naga::AddressSpace::Storage {
                                access: access(crate::CONSTANTS_BINDING_READ_ONLY),
                            },
                            binding: Some(naga::ResourceBinding {
                                group: 0,
                                binding: crate::CONSTANTS_BINDING_INDEX,
                            }),
                            ty: flags_array_ty,
                            init: None,
                        },
                        naga::Span::UNDEFINED,
                    );
                    $(
                        let $name = make_word_binding(
                            module,
                            word_array_ty,
                            concat!("wasm_", stringify!($name)),
                            crate::[< $name:upper _BINDING_READ_ONLY >],
                            crate::[< $name:upper _BINDING_INDEX >],
                        )?;
                    )*

                    Ok(Self {
                        flags,
                        constants,
                        $($name),*
                    })
                }
            }
        }
    };
}

word_bindings! {
    struct StdBindings {
        flags, constants, memory, mutable_globals, immutable_globals, tables, data, elements, input, output, stack
    }
}
