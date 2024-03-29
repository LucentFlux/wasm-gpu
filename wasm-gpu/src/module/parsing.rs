use crate::module::error::WasmError;
use ouroboros::self_referencing;
use std::collections::HashMap;
use std::ops::Range;
use wasm_opcodes::OperatorByProposal;
use wasmparser::{
    BinaryReaderError, DataKind, ElementKind, Encoding, ExternalKind, FuncType,
    FuncValidatorAllocations, GlobalType, MemoryType, NameSectionReader, Operator, Parser, Payload,
    RefType, Table, TableType, Type, TypeRef, ValType, Validator,
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

pub struct ParsedFunc {
    pub type_id: u32,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<OperatorByProposal>,
}

pub enum ParsedElementKind<'data> {
    /// The element segment is passive.
    Passive,
    /// The element segment is active.
    Active {
        /// The index of the table being initialized.
        table_index: Option<u32>,
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
    pub ty: RefType,
    /// The range of the the element segment.
    pub range: Range<usize>,
}

pub enum ParsedDataKind<'data> {
    /// The data segment is passive.
    Passive,
    /// The data segment is active.
    Active {
        /// The memory index for the data segment.
        memory_index: u32,
        /// The initialization expression for the data segment.
        offset_expr: Vec<Operator<'data>>,
    },
}

pub struct ParsedData<'data> {
    /// The kind of data segment.
    pub kind: ParsedDataKind<'data>,
    /// The data of the data segment.
    pub data: &'data [u8],
    /// The range of the data segment.
    pub range: Range<usize>,
}

#[derive(Debug)]
pub struct ParsedTable<'data> {
    /// The type of this table, including its element type and its limits.
    pub ty: TableType,
    /// The initialization expression for the table.
    pub init: ParsedTableInit<'data>,
}

/// Different modes of initializing a table.
#[derive(Debug)]
pub enum ParsedTableInit<'data> {
    /// The table is initialized to all null elements.
    RefNull,
    /// Each element in the table is initialized with the specified constant
    /// expression.
    Expr(Vec<Operator<'data>>),
}

pub struct ParsedModule<'data> {
    pub types: Vec<FuncType>,
    pub imports: Vec<(&'data str, &'data str, ImportTypeRef)>,
    pub tables: Vec<ParsedTable<'data>>,
    pub memories: Vec<MemoryType>,
    pub globals: Vec<Global<'data>>,
    pub exports: HashMap<String, ModuleExport>,
    pub start_func: Option<u32>,
    pub elements: Vec<ParsedElement<'data>>,
    pub datas: Vec<ParsedData<'data>>,
    pub functions: Vec<ParsedFunc>,
}

#[self_referencing]
pub struct ParsedModuleUnit {
    pub src: Vec<u8>,

    #[borrows(src)]
    #[covariant]
    pub sections: ParsedModule<'this>,
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

    pub fn translate(mut self, parser: Parser, src: Vec<u8>) -> WasmResult<ParsedModuleUnit> {
        let unit = ParsedModuleUnit::try_new(src, |src| {
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

            let parsed = parser.parse_all(src.as_slice());
            for payload in parsed {
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

            Ok::<_, WasmError>(result)
        })?;

        return Ok(unit);
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
                let num = usize::try_from(types.count()).unwrap();
                result.types.reserve(num);

                for ty in types {
                    let Type::Func(ty) = ty?;
                    result.types.push(ty);
                }
            }

            Payload::ImportSection(imports) => {
                self.validator.import_section(&imports)?;

                let cnt = usize::try_from(imports.count()).unwrap();
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

                let cnt = usize::try_from(functions.count()).unwrap();
                scratch.function_types.reserve(cnt);

                for entry in functions {
                    scratch.function_types.push(entry?)
                }
            }

            Payload::TableSection(tables) => {
                self.validator.table_section(&tables)?;

                let cnt = usize::try_from(tables.count()).unwrap();
                result.tables.reserve(cnt);

                for entry in tables {
                    let Table { ty, init } = entry?;
                    result.tables.push(ParsedTable {
                        ty,
                        init: match init {
                            wasmparser::TableInit::RefNull => ParsedTableInit::RefNull,
                            wasmparser::TableInit::Expr(expr) => ParsedTableInit::Expr(
                                expr.get_operators_reader()
                                    .into_iter()
                                    .collect::<Result<_, _>>()?,
                            ),
                        },
                    });
                }
            }

            Payload::MemorySection(memories) => {
                self.validator.memory_section(&memories)?;

                let cnt = usize::try_from(memories.count()).unwrap();
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

                let cnt = usize::try_from(globals.count()).unwrap();
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

                let cnt = usize::try_from(exports.count()).unwrap();
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

                let cnt = usize::try_from(elements.count()).unwrap();
                result.elements.reserve(cnt);

                for element in elements {
                    let element = element?;

                    // Parse values
                    let cnt = match &element.items {
                        wasmparser::ElementItems::Functions(items_reader) => items_reader.count(),
                        wasmparser::ElementItems::Expressions(items_reader) => items_reader.count(),
                    };
                    let cnt = usize::try_from(cnt).unwrap();

                    let mut items = Vec::new();
                    items.reserve(cnt);

                    match element.items {
                        wasmparser::ElementItems::Functions(items_reader) => {
                            for item in items_reader {
                                items.push(vec![
                                    Operator::RefFunc {
                                        function_index: item?,
                                    },
                                    Operator::End,
                                ])
                            }
                        }
                        wasmparser::ElementItems::Expressions(items_reader) => {
                            for expr in items_reader {
                                let expr: Result<Vec<Operator>, BinaryReaderError> =
                                    expr?.get_operators_reader().into_iter().collect();
                                items.push(expr?)
                            }
                        }
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

            Payload::CodeSectionEntry(body) => {
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
                    let op = OperatorByProposal::from_operator(operator?)?;

                    func.operators.push(op);
                }

                result.functions.push(func);
            }

            Payload::DataCountSection { count, range } => {
                self.validator.data_count_section(count, &range)?;

                let cnt = usize::try_from(count).unwrap();
                result.datas.reserve(cnt);
            }

            Payload::DataSection(data) => {
                self.validator.data_section(&data)?;

                for data in data {
                    let data = data?;

                    let kind = match data.kind {
                        DataKind::Passive => ParsedDataKind::Passive,
                        DataKind::Active {
                            memory_index,
                            offset_expr,
                        } => {
                            let offset_expr: Result<Vec<Operator>, BinaryReaderError> =
                                offset_expr.get_operators_reader().into_iter().collect();
                            ParsedDataKind::Active {
                                memory_index,
                                offset_expr: offset_expr?,
                            }
                        }
                    };

                    result.datas.push(ParsedData {
                        kind,
                        data: data.data,
                        range: data.range,
                    });
                }
            }

            Payload::CustomSection(s) => {
                match s.name() {
                    "name" => {
                        let _reader = NameSectionReader::new(s.data(), 0);
                    }
                    _ => {
                        return Err(WasmError::Unsupported(format!(
                            "custom section not supported: {}",
                            s.name()
                        )));
                    }
                };
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
