//! Provide a collection of shorthand and opinionated methods extending base naga objects.

use wasmparser::ValType;

use crate::std_objects::StdObjects;

mod sealed {
    pub trait ModuleSealed {}
    impl ModuleSealed for naga::Module {}
    pub trait FunctionsSealed {}
    impl FunctionsSealed for naga::Arena<naga::Function> {}
    pub trait TypesSealed {}
    impl TypesSealed for naga::UniqueArena<naga::Type> {}
    pub trait FunctionSealed {}
    impl FunctionSealed for naga::Function {}
}

pub(crate) trait ModuleExt: self::sealed::ModuleSealed {
    /// Shorthand on fields
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function>;
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
    fn fn_mut(&mut self, handle: naga::Handle<naga::Function>) -> &mut naga::Function;
}

impl ModuleExt for naga::Module {
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function> {
        self.functions.new_empty_function(name)
    }
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function> {
        self.functions.new_function(definition)
    }

    fn fn_mut(&mut self, handle: naga::Handle<naga::Function>) -> &mut naga::Function {
        self.functions.get_mut(handle)
    }
}

pub(crate) trait FunctionsExt: self::sealed::FunctionsSealed {
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function>;
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
}

impl FunctionsExt for naga::Arena<naga::Function> {
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function> {
        let mut new_function = naga::Function::default();
        new_function.name = Some(name);

        let handle = self.append(new_function, naga::Span::UNDEFINED);
        return handle;
    }
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function> {
        // Populate function definition from the signature, assuming the function isn't a boundary function
        let mut new_function = naga::Function::default();
        new_function.name = Some(definition.name);
        new_function.arguments = definition
            .args
            .into_iter()
            .map(|(name, ty)| naga::FunctionArgument {
                name: Some(name),
                ty,
                binding: None,
            })
            .collect();
        new_function.result = definition
            .result
            .map(|ty| naga::FunctionResult { ty, binding: None });

        let handle = self.append(new_function, naga::Span::UNDEFINED);
        return handle;
    }
}

pub(crate) trait TypesExt: self::sealed::TypesSealed {}

impl TypesExt for naga::UniqueArena<naga::Type> {}

pub(crate) trait FunctionExt: self::sealed::FunctionSealed {
    // Shorthand handle generation
    fn new_local(
        &mut self,
        name: String,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
    ) -> naga::Handle<naga::LocalVariable>;
    fn new_wasm_local(
        &mut self,
        name: String,
        val_ty: ValType,
        std_objects: &StdObjects,
    ) -> naga::Handle<naga::LocalVariable>;

    // Shorthand expression generation
    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression>;
    fn append_fn_argument(&mut self, argument_index: u32) -> naga::Handle<naga::Expression>;
    fn append_local(
        &mut self,
        local: naga::Handle<naga::LocalVariable>,
    ) -> naga::Handle<naga::Expression>;
    fn append_compose_push_emit(
        &mut self,
        ty: naga::Handle<naga::Type>,
        components: Vec<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::Expression>;

    // Shorthand statement addition
    fn push_emit(&mut self, expression: naga::Handle<naga::Expression>);
    fn push_return(&mut self, expression: naga::Handle<naga::Expression>);
    fn push_store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    );
}
impl FunctionExt for naga::Function {
    fn new_local(
        &mut self,
        name: String,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
    ) -> naga::Handle<naga::LocalVariable> {
        self.local_variables.append(
            naga::LocalVariable {
                name: Some(name),
                ty,
                init,
            },
            naga::Span::UNDEFINED,
        )
    }

    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.expressions.append(
            naga::Expression::GlobalVariable(global),
            naga::Span::UNDEFINED,
        )
    }
    fn append_fn_argument(&mut self, argument_index: u32) -> naga::Handle<naga::Expression> {
        self.expressions.append(
            naga::Expression::FunctionArgument(argument_index),
            naga::Span::UNDEFINED,
        )
    }
    fn append_local(
        &mut self,
        local: naga::Handle<naga::LocalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.expressions.append(
            naga::Expression::LocalVariable(local),
            naga::Span::UNDEFINED,
        )
    }
    fn append_compose_push_emit(
        &mut self,
        ty: naga::Handle<naga::Type>,
        components: Vec<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::Expression> {
        let expression = self.expressions.append(
            naga::Expression::Compose { ty, components },
            naga::Span::UNDEFINED,
        );
        self.push_emit(expression);

        return expression;
    }

    fn push_emit(&mut self, expression: naga::Handle<naga::Expression>) {
        self.body.push(
            naga::Statement::Emit(naga::Range::new_from_bounds(expression, expression)),
            naga::Span::UNDEFINED,
        );
    }
    fn push_return(&mut self, expression: naga::Handle<naga::Expression>) {
        self.body.push(
            naga::Statement::Return {
                value: Some(expression),
            },
            naga::Span::UNDEFINED,
        );
    }
    fn push_store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    ) {
        self.body.push(
            naga::Statement::Store { pointer, value },
            naga::Span::UNDEFINED,
        );
    }

    fn new_wasm_local(
        &mut self,
        name: String,
        val_ty: ValType,
        std_objects: &StdObjects,
    ) -> naga::Handle<naga::LocalVariable> {
        todo!()
    }
}

pub(crate) struct FunctionSignature {
    pub name: String,
    pub args: Vec<(String, naga::Handle<naga::Type>)>,
    pub result: Option<naga::Handle<naga::Type>>,
}

/// Provides an inline way of defining functions and getting their arguments as expressions.
///
/// # Usage
///
/// ```ignore
/// let (handle, word_address) = declare_function! {
///     module => fn read_i32(word_address: address_ty) -> i32_ty
/// };
/// ```
/// or
///
/// ```ignore
/// let name = "read_i32";
/// let (handle, word_address) = declare_function! {
///     module => fn {name}(word_address: address_ty) -> i32_ty
/// };
/// ```
#[macro_export]
macro_rules! declare_function {
    (@match_fn_name {$fn_name_var:expr}) => {$fn_name_var.to_owned()};
    (@match_fn_name $fn_name:tt) => {stringify!{$fn_name}.to_owned()};

    ($module:expr => fn $fn_name:tt ( $($arg_name:ident : $arg_ty:tt),* $(,)? ) $(-> $ret_ty:tt)?) => {{
        let mut result = None;
        $(
            result = Some($ret_ty);
        )?

        let function_signature = $crate::module_ext::FunctionSignature {
            name: $crate::declare_function!(@match_fn_name $fn_name),
            args: vec![
                $(
                    (stringify!{$arg_name}.to_owned(), $arg_ty),
                )*
            ],
            result,
        };

        let function_handle = $crate::module_ext::ModuleExt::new_function($module, function_signature);

        let function = $module.functions.get_mut(function_handle.clone());

        (function_handle, $(
            function.expressions.append(naga::Expression::FunctionArgument(${ignore(arg_name)} ${index()}), naga::Span::UNDEFINED),
        )*)
    }};
}

#[macro_export]
macro_rules! naga_expr {
    ($active_function:expr => $($expression:tt)*) => {{
        let (mut module, function) = $active_function.get_active();
        #[allow(unused)]
        let constants = &mut module.constants;
        let expressions = &mut function.expressions;
        let block = &mut function.body;
        $crate::naga_expr!(@inner constants, expressions, block => $($expression)*)
    }};

    ($module:expr, $function_handle:expr => $($expression:tt)*) => {{
        let function = $module.functions.get_mut($function_handle.clone());
        #[allow(unused)]
        let constants = &mut $module.constants;
        let expressions = &mut function.expressions;
        let block = &mut function.body;
        $crate::naga_expr!(@inner constants, expressions, block => $($expression)*)
    }};

    ($module:expr, $function_handle:expr, $block:expr => $($expression:tt)*) => {{
        let function = $module.functions.get_mut($function_handle.clone());
        #[allow(unused)]
        let constants = &mut $module.constants;
        let expressions = &mut function.expressions;
        $crate::naga_expr!(@inner constants, expressions, $block => $($expression)*)
    }};

    (@emit $block:expr => $handle:ident) => {{
        $block
            .push(naga::Statement::Emit(naga::Range::new_from_bounds(
                $handle, $handle,
            )), naga::Span::UNDEFINED);
        $handle
    }};

    // Casting
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt as $kind:tt $($others:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let handle = $expressions.append(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: None }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Bin Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt + $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Add, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt - $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Subtract, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt * $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Multiply, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt / $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Divide, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle)
    }};

    // Struct Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $base:tt [const $index:expr ] $($others:tt)*) => {{
        let base = naga_expr!(@inner $constants, $expressions, $block => $base);
        let handle = $expressions.append(naga::Expression::AccessIndex{ base, index: $index }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Array Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $base:tt [ $($index:tt)* ] $($others:tt)*) => {{
        let base = naga_expr!(@inner $constants, $expressions, $block => $base);
        let index = naga_expr!(@inner $constants, $expressions, $block => $($index)*);
        let handle = $expressions.append(naga::Expression::Access { base, index }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Constants
    (@inner $constants:expr, $expressions:expr, $block:expr => I32($term:expr) $($others:tt)*) => {{
        let const_handle = $constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Sint($term as i64) },
        }, naga::Span::UNDEFINED);
        let handle = $expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => U32($term:expr) $($others:tt)*) => {{
        let const_handle = $constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Uint($term as u64) },
        }, naga::Span::UNDEFINED);
        let handle = $expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => Global($term:expr) $($others:tt)*) => {{
        let handle = expressions.append(naga::Expression::GlobalVariable($term), naga::Span::UNDEFINED);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Deref
    (@inner $constants:expr, $expressions:expr, $block:expr => Load($($pointer:tt)*) $($others:tt)*) => {{
        let pointer = naga_expr!(@inner $constants, $expressions, $block => $($pointer)*);
        let handle = $expressions.append(naga::Expression::Load { pointer }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Constructors
    (@innerconstructor $constants:expr, $expressions:expr, $block:expr, $components:expr => $e1:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $constants, $expressions, $block => $e1));
        $(naga_expr!(@innerconstructor $constants, $expressions, $block, $components => $($others)*);)*
    }};
    (@innerconstructor $constants:expr, $expressions:expr, $block:expr, $components:expr => $e1:tt $e2:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $constants, $expressions, $block => $e1 $e2));
        $(naga_expr!(@innerconstructor $constants, $expressions, $block, $components => $($others)*);)*
    }};
    (@innerconstructor $constants:expr, $expressions:expr, $block:expr, $components:expr => $e1:tt $e2:tt $e3:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $constants, $expressions, $block => $e1 $e2 $e3));
        $(naga_expr!(@innerconstructor $constants, $expressions, $block, $components => $($others)*);)*
    }};
    (@innerconstructor $constants:expr, $expressions:expr, $block:expr, $components:expr => $e1:tt $e2:tt $e3:tt $e4:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $constants, $expressions, $block => $e1 $e2 $e3 $e4));
        $(naga_expr!(@innerconstructor $constants, $expressions, $block, $components => $($others)*);)*
    }};
    (@innerconstructor $constants:expr, $expressions:expr, $block:expr, $components:expr => $e1:tt $e2:tt $e3:tt $e4:tt $e5:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $constants, $expressions, $block => $e1 $e2 $e3 $e4 $e5));
        $(naga_expr!(@innerconstructor $constants, $expressions, $block, $components => $($others)*);)*
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $ty:tt ( $($args:tt)* ) $($others:tt)*) => {{
        let mut components = Vec::new();
        naga_expr!{@innerconstructor $constants, $expressions, $block, components => $($args)* }
        let handle = $expressions.append(
            naga::Expression::Compose {ty: $ty, components},
            naga::Span::UNDEFINED,
        );
        $crate::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Step braces
    (@inner $constants:expr, $expressions:expr, $block:expr => ($($expression:tt)*) $($others:tt)*) => {{
        let lhs = $crate::naga_expr!(@inner $constants, $expressions, $block => $($expression)*);
        naga_expr!(@inner $constants, $expressions, $block => lhs $($others)*)
    }};

    // Arbitrary embeddings
    (@inner $constants:expr, $expressions:expr, $block:expr => $term:expr) => { $term };
}
