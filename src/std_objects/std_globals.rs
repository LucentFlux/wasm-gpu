mod bindings;

use std::marker::PhantomData;

use once_cell::sync::OnceCell;
use perfect_derive::perfect_derive;

use crate::build;

use super::Generator;

/// A global that attaches itself to a module the first time it is requested
pub(crate) trait GlobalGen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>>;
}

#[perfect_derive(Default)]
pub(crate) struct LazyGlobal<I: GlobalGen> {
    handle: OnceCell<build::Result<naga::Handle<naga::GlobalVariable>>>,
    _phantom: PhantomData<I>,
}

impl<I: GlobalGen> Generator for LazyGlobal<I> {
    type Generated = naga::Handle<naga::GlobalVariable>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        self.handle.get_or_init(|| I::gen(module, others)).clone()
    }
}

macro_rules! std_bindings {
    (struct $gen_struct_name:ident { $($name:ident),* $(,)? } => $vis:vis struct $fin_struct_name:ident;) => {
        paste::paste!{
            #[perfect_derive(Default)]
            pub(super) struct $gen_struct_name {
                $(
                    pub(super) $name: LazyGlobal<bindings::[<$name:camel BindingGen>]>,
                )*
            }

            impl Generator for $gen_struct_name {
                type Generated = $fin_struct_name;

                fn gen<Ps: super::GenerationParameters>(
                    &self,
                    module: &mut naga::Module,
                    others: &super::StdObjectsGenerator<Ps>,
                ) -> crate::build::Result<Self::Generated> {
                    $(
                        let $name = self.$name.gen(module, others)?;
                    )*

                    Ok($fin_struct_name {
                        $($name),*
                    })
                }
            }

            #[derive(Clone)]
            $vis struct $fin_struct_name {
                $(
                    $vis $name: naga::Handle<naga::GlobalVariable>,
                )*
            }
        }
    };
}

std_bindings! {
    struct StdBindingsGenerator {
        memory, mutable_globals, immutable_globals, tables, data, elements, input, output, stack, flags
    } => pub(crate) struct StdBindings;
}
