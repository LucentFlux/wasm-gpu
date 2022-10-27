use crate::module::error::WasmError;
use std::collections::HashMap;
use std::ops::Range;
use wasmparser::{
    BinaryReaderError, Data, ElementItem, ElementKind, Encoding, ExternalKind,
    FuncValidatorAllocations, GlobalType, MemoryType, Operator, Parser, Payload, TableType, Type,
    TypeRef, ValType, Validator,
};

type WasmResult<T> = Result<T, WasmError>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ImportTypeRef {
    Func(u32),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
}

#[derive(Debug, Clone)]
pub struct Global<'data> {
    pub ty: GlobalType,
    pub initializer: Vec<Operator<'data>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ModuleExport {
    Func(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

pub struct ParsedFunc<'data> {
    pub type_id: u32,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<Operator<'data>>,
}

pub enum ParsedElementKind<'data> {
    /// The element segment is passive.
    Passive,
    /// The element segment is active.
    Active {
        /// The index of the table being initialized.
        table_index: u32,
        /// The initial expression of the element segment.
        offset_expr: Vec<Operator<'data>>,
    },
    /// The element segment is declared.
    Declared,
}

pub struct ParsedElement<'data> {
    pub kind: ParsedElementKind<'data>,
    /// The initial elements of the element segment.
    pub items: Vec<Vec<Operator<'data>>>,
    /// The type of the elements.
    pub ty: ValType,
    /// The range of the the element segment.
    pub range: Range<usize>,
}

pub struct ParsedModule<'data> {
    pub types: Vec<Type>,
    pub imports: Vec<(&'data str, &'data str, ImportTypeRef)>,
    pub tables: Vec<TableType>,
    pub memories: Vec<MemoryType>,
    pub globals: Vec<Global<'data>>,
    pub exports: HashMap<String, ModuleExport>,
    pub start_func: Option<u32>,
    pub elements: Vec<ParsedElement<'data>>,
    pub datas: Vec<Data<'data>>,
    pub functions: Vec<ParsedFunc<'data>>,
}

struct IntermediateData {
    function_types: Vec<u32>,
    allocs: Option<FuncValidatorAllocations>,
}

pub struct ModuleEnviron {
    validator: Validator,
}

impl ModuleEnviron {
    pub fn new(validator: Validator) -> Self {
        Self { validator }
    }

    pub fn translate(mut self, parser: Parser, data: &[u8]) -> WasmResult<ParsedModule> {
        let mut result = ParsedModule {
            types: vec![],
            imports: vec![],
            tables: vec![],
            memories: vec![],
            globals: vec![],
            exports: Default::default(),
            start_func: None,
            elements: vec![],
            datas: vec![],
            functions: vec![],
        };
        let mut scratch = IntermediateData {
            function_types: vec![],
            allocs: None,
        };

        for payload in parser.parse_all(data) {
            self.translate_payload(payload?, &mut scratch, &mut result)?;
        }

        // Minimise space - we presumably won't be doing anything more
        result.types.shrink_to_fit();
        result.imports.shrink_to_fit();
        result.tables.shrink_to_fit();
        result.memories.shrink_to_fit();
        result.globals.shrink_to_fit();
        result.exports.shrink_to_fit();
        result.elements.shrink_to_fit();
        result.datas.shrink_to_fit();
        result.functions.shrink_to_fit();

        Ok(result)
    }

    fn translate_payload<'data>(
        &mut self,
        payload: Payload<'data>,
        scratch: &mut IntermediateData,
        result: &mut ParsedModule<'data>,
    ) -> WasmResult<()> {
        match payload {
            Payload::Version {
                num,
                encoding,
                range,
            } => {
                self.validator.version(num, encoding, &range)?;
                match encoding {
                    Encoding::Module => {}
                    Encoding::Component => {
                        return Err(WasmError::Unsupported(format!("component model")));
                    }
                }
            }

            Payload::End(offset) => {
                self.validator.end(offset)?;
            }

            Payload::TypeSection(types) => {
                self.validator.type_section(&types)?;
                let num = usize::try_from(types.get_count()).unwrap();
                result.types.reserve(num);

                for ty in types {
                    result.types.push(ty?);
                }
            }

            Payload::ImportSection(imports) => {
                self.validator.import_section(&imports)?;

                let cnt = usize::try_from(imports.get_count()).unwrap();
                result.imports.reserve(cnt);

                for entry in imports {
                    let import = entry?;
                    let ty = match import.ty {
                        TypeRef::Func(index) => ImportTypeRef::Func(index),
                        TypeRef::Memory(ty) => ImportTypeRef::Memory(ty),
                        TypeRef::Global(ty) => ImportTypeRef::Global(ty),
                        TypeRef::Table(ty) => ImportTypeRef::Table(ty),

                        TypeRef::Tag(_) => {
                            return Err(WasmError::Unsupported(format!("exceptions")))
                        }
                    };
                    result.imports.push((import.module, import.name, ty));
                }
            }

            Payload::FunctionSection(functions) => {
                self.validator.function_section(&functions)?;

                let cnt = usize::try_from(functions.get_count()).unwrap();
                scratch.function_types.reserve(cnt);

                for entry in functions {
                    scratch.function_types.push(entry?)
                }
            }

            Payload::TableSection(tables) => {
                self.validator.table_section(&tables)?;

                let cnt = usize::try_from(tables.get_count()).unwrap();
                result.tables.reserve(cnt);

                for entry in tables {
                    result.tables.push(entry?);
                }
            }

            Payload::MemorySection(memories) => {
                self.validator.memory_section(&memories)?;

                let cnt = usize::try_from(memories.get_count()).unwrap();
                result.memories.reserve(cnt);

                for entry in memories {
                    result.memories.push(entry?);
                }
            }

            Payload::TagSection(tags) => {
                self.validator.tag_section(&tags)?;

                // We don't support exceptions
                return Err(WasmError::Unsupported(format!("exceptions")));
            }

            Payload::GlobalSection(globals) => {
                self.validator.global_section(&globals)?;

                let cnt = usize::try_from(globals.get_count()).unwrap();
                result.globals.reserve(cnt);

                for entry in globals {
                    let wasmparser::Global { ty, init_expr } = entry?;
                    let mut init_expr_reader = init_expr.get_binary_reader();
                    let mut initializer = Vec::new();
                    while !init_expr_reader.eof() {
                        initializer.push(init_expr_reader.read_operator()?)
                    }
                    let ty = Global { ty, initializer };
                    result.globals.push(ty);
                }
            }

            Payload::ExportSection(exports) => {
                self.validator.export_section(&exports)?;

                let cnt = usize::try_from(exports.get_count()).unwrap();
                result.exports.reserve(cnt);

                for entry in exports {
                    let wasmparser::Export { name, kind, index } = entry?;
                    let entity = match kind {
                        ExternalKind::Func => ModuleExport::Func(index as usize),
                        ExternalKind::Table => ModuleExport::Table(index as usize),
                        ExternalKind::Memory => ModuleExport::Memory(index as usize),
                        ExternalKind::Global => ModuleExport::Global(index as usize),

                        // this never gets past validation
                        ExternalKind::Tag => {
                            return Err(WasmError::Unsupported(format!("exceptions")))
                        }
                    };
                    result.exports.insert(String::from(name), entity);
                }
            }

            Payload::StartSection { func, range } => {
                self.validator.start_section(func, &range)?;

                assert!(result.start_func.is_none());
                result.start_func = Some(func);
            }

            Payload::ElementSection(elements) => {
                self.validator.element_section(&elements)?;

                let cnt = usize::try_from(elements.get_count()).unwrap();
                result.elements.reserve(cnt);

                for element in elements {
                    let element = element?;

                    // Parse values
                    let items_reader = element.items.get_items_reader()?;
                    let cnt = usize::try_from(items_reader.get_count()).unwrap();

                    let items = if items_reader.uses_exprs() {
                        let mut items = Vec::new();
                        items.reserve(cnt);

                        for item in items_reader {
                            match item? {
                                ElementItem::Expr(expr) => {
                                    let expr: Result<Vec<Operator>, BinaryReaderError> =
                                        expr.get_operators_reader().into_iter().collect();
                                    items.push(expr?)
                                }
                                _ => unreachable!(),
                            }
                        }

                        items
                    } else {
                        let mut items = Vec::new();
                        items.reserve(cnt);

                        for item in items_reader {
                            match item? {
                                ElementItem::Func(f) => items.push(vec![
                                    Operator::RefFunc { function_index: f },
                                    Operator::End,
                                ]),
                                _ => unreachable!(),
                            }
                        }

                        items
                    };

                    let kind = match element.kind {
                        ElementKind::Passive => ParsedElementKind::Passive,
                        ElementKind::Active {
                            table_index,
                            offset_expr,
                        } => {
                            let offset_expr: Result<Vec<Operator>, BinaryReaderError> =
                                offset_expr.get_operators_reader().into_iter().collect();
                            ParsedElementKind::Active {
                                table_index,
                                offset_expr: offset_expr?,
                            }
                        }
                        ElementKind::Declared => ParsedElementKind::Declared,
                    };

                    result.elements.push(ParsedElement {
                        kind,
                        items,
                        ty: element.ty,
                        range: element.range,
                    });
                }
            }

            Payload::CodeSectionStart { count, range, .. } => {
                self.validator.code_section_start(count, &range)?;
                let cnt = usize::try_from(count).unwrap();
                result.functions.reserve(cnt);
            }

            Payload::CodeSectionEntry(mut body) => {
                let validator = self.validator.code_section_entry(&body)?;
                let mut validator =
                    validator.into_validator(scratch.allocs.take().unwrap_or_default());
                validator.validate(&body)?;
                scratch.allocs = Some(validator.into_allocations());

                let func_id = result.functions.len();
                let type_id = scratch.function_types.get(func_id).expect("function types vec was not large enough - this should have been caught at validation");

                let mut func = ParsedFunc {
                    locals: vec![],
                    operators: vec![],
                    type_id: type_id.clone(),
                };
                for local in body.get_locals_reader()? {
                    func.locals.push(local?);
                }
                for operator in body.get_operators_reader()? {
                    func.operators.push(operator?);
                }

                result.functions.push(func);
            }

            Payload::DataCountSection { count, range } => {
                self.validator.data_count_section(count, &range)?;

                let cnt = usize::try_from(count).unwrap();
                result.datas.reserve(cnt);
            }

            Payload::DataSection(mut data) => {
                self.validator.data_section(&data)?;

                let data = data.read()?;

                result.datas.push(data)
            }

            Payload::CustomSection(s) => {
                return Err(WasmError::Unsupported(format!(
                    "custom section not supported: {}",
                    s.name()
                )));
            }

            // It's expected that validation will probably reject other
            // payloads such as `UnknownSection` or those related to the
            // component model. If, however, something gets past validation then
            // that's a bug in this as we forgot to implement something.
            other => {
                self.validator.payload(&other)?;
                panic!("unimplemented section in wasm file {:?}", other);
            }
        }
        Ok(())
    }
}
