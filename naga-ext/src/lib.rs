//! Provide a collection of shorthand and opinionated methods extending base naga objects.
pub mod block_context;
pub mod into_literal;
pub use block_context::BlockContext;

use sealed::sealed;

#[sealed]
pub trait ModuleExt {
    /// Shorthand for `module.functions.new_empty_function(name)`
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function>;
    /// Shorthand for `module.functions.new_function(definition)`
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
    /// Shorthand for `module.functions.get_mut(handle)`
    fn fn_mut(&mut self, handle: naga::Handle<naga::Function>) -> &mut naga::Function;
}

#[sealed]
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

#[sealed]
pub trait FunctionsExt {
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function>;
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
}

#[sealed]
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

#[sealed]
pub trait ConstantsExt {
    fn append_anonymous(
        &mut self,
        ty: naga::Handle<naga::Type>,
        init: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Constant>;
}

#[sealed]
impl ConstantsExt for naga::Arena<naga::Constant> {
    fn append_anonymous(
        &mut self,
        ty: naga::Handle<naga::Type>,
        init: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Constant> {
        self.append(
            naga::Constant {
                name: None,
                ty,
                r#override: naga::Override::None,
                init,
            },
            naga::Span::UNDEFINED,
        )
    }
}

#[sealed]
pub trait TypesExt {
    fn insert_anonymous(&mut self, ty: naga::TypeInner) -> naga::Handle<naga::Type>;

    fn insert_scalar(&mut self, scalar: naga::Scalar) -> naga::Handle<naga::Type>;

    fn insert_i32(&mut self) -> naga::Handle<naga::Type>;
    fn insert_i64(&mut self) -> naga::Handle<naga::Type>;
    fn insert_u32(&mut self) -> naga::Handle<naga::Type>;
    fn insert_f32(&mut self) -> naga::Handle<naga::Type>;
    fn insert_f64(&mut self) -> naga::Handle<naga::Type>;

    fn insert_bool(&mut self) -> naga::Handle<naga::Type>;
}

#[sealed]
impl TypesExt for naga::UniqueArena<naga::Type> {
    fn insert_anonymous(&mut self, ty: naga::TypeInner) -> naga::Handle<naga::Type> {
        self.insert(
            naga::Type {
                name: None,
                inner: ty,
            },
            naga::Span::UNDEFINED,
        )
    }

    fn insert_scalar(&mut self, scalar: naga::Scalar) -> naga::Handle<naga::Type> {
        self.insert_anonymous(naga::TypeInner::Scalar(scalar))
    }

    fn insert_i32(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::I32)
    }
    fn insert_i64(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::I64)
    }
    fn insert_u32(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::U32)
    }
    fn insert_f32(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::F32)
    }
    fn insert_f64(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::F64)
    }

    fn insert_bool(&mut self) -> naga::Handle<naga::Type> {
        self.insert_scalar(naga::Scalar::BOOL)
    }
}

#[sealed]
pub trait FunctionExt {}

#[sealed]
impl FunctionExt for naga::Function {}

#[sealed]
pub trait LocalsExt {
    // Shorthand handle generation
    fn new_local(
        &mut self,
        name: impl Into<String>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::LocalVariable>;
}

#[sealed]
impl LocalsExt for naga::Arena<naga::LocalVariable> {
    fn new_local(
        &mut self,
        name: impl Into<String>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::LocalVariable> {
        self.append(
            naga::LocalVariable {
                name: Some(name.into()),
                ty,
                init,
            },
            naga::Span::UNDEFINED,
        )
    }
}

#[sealed]
pub trait ExpressionsExt {
    // Shorthand expression generation
    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression>;
    fn append_constant(
        &mut self,
        constant: naga::Handle<naga::Constant>,
    ) -> naga::Handle<naga::Expression>;
    fn append_fn_argument(&mut self, argument_index: u32) -> naga::Handle<naga::Expression>;
    fn append_local(
        &mut self,
        local: naga::Handle<naga::LocalVariable>,
    ) -> naga::Handle<naga::Expression>;
    fn append_compose(
        &mut self,
        ty: naga::Handle<naga::Type>,
        components: Vec<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::Expression>;
    fn append_load(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression>;
    fn append_literal(&mut self, literal: naga::Literal) -> naga::Handle<naga::Expression>;

    fn append_u32(&mut self, value: u32) -> naga::Handle<naga::Expression>;
    fn append_i32(&mut self, value: i32) -> naga::Handle<naga::Expression>;
    fn append_i64(&mut self, value: i64) -> naga::Handle<naga::Expression>;
    fn append_f32(&mut self, value: f32) -> naga::Handle<naga::Expression>;
    fn append_f64(&mut self, value: f64) -> naga::Handle<naga::Expression>;
    fn append_bool(&mut self, value: bool) -> naga::Handle<naga::Expression>;
}

#[sealed]
impl ExpressionsExt for naga::Arena<naga::Expression> {
    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.append(
            naga::Expression::GlobalVariable(global),
            naga::Span::UNDEFINED,
        )
    }
    fn append_constant(
        &mut self,
        constant: naga::Handle<naga::Constant>,
    ) -> naga::Handle<naga::Expression> {
        self.append(naga::Expression::Constant(constant), naga::Span::UNDEFINED)
    }
    fn append_fn_argument(&mut self, argument_index: u32) -> naga::Handle<naga::Expression> {
        self.append(
            naga::Expression::FunctionArgument(argument_index),
            naga::Span::UNDEFINED,
        )
    }
    fn append_local(
        &mut self,
        local: naga::Handle<naga::LocalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.append(
            naga::Expression::LocalVariable(local),
            naga::Span::UNDEFINED,
        )
    }
    fn append_compose(
        &mut self,
        ty: naga::Handle<naga::Type>,
        components: Vec<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::Expression> {
        self.append(
            naga::Expression::Compose { ty, components },
            naga::Span::UNDEFINED,
        )
    }
    fn append_load(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression> {
        self.append(naga::Expression::Load { pointer }, naga::Span::UNDEFINED)
    }

    fn append_literal(&mut self, literal: naga::Literal) -> naga::Handle<naga::Expression> {
        self.append(naga::Expression::Literal(literal), naga::Span::UNDEFINED)
    }
    fn append_u32(&mut self, value: u32) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::U32(value))
    }
    fn append_i32(&mut self, value: i32) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::I32(value))
    }
    fn append_i64(&mut self, value: i64) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::I64(value))
    }
    fn append_f32(&mut self, value: f32) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::F32(value))
    }
    fn append_f64(&mut self, value: f64) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::F64(value))
    }
    fn append_bool(&mut self, value: bool) -> naga::Handle<naga::Expression> {
        self.append_literal(naga::Literal::Bool(value))
    }
}

#[sealed]
pub trait BlockExt {
    // Shorthand statement addition
    fn push_emit(&mut self, expression: naga::Handle<naga::Expression>);
    fn push_return(&mut self, expression: naga::Handle<naga::Expression>);
    fn push_bare_return(&mut self);
    fn push_store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    );
    fn push_if(
        &mut self,
        condition: naga::Handle<naga::Expression>,
        accept: naga::Block,
        reject: naga::Block,
    );
    fn push_kill(&mut self);
}

#[sealed]
impl BlockExt for naga::Block {
    fn push_emit(&mut self, expression: naga::Handle<naga::Expression>) {
        self.push(
            naga::Statement::Emit(naga::Range::new_from_bounds(expression, expression)),
            naga::Span::UNDEFINED,
        );
    }
    fn push_return(&mut self, expression: naga::Handle<naga::Expression>) {
        self.push(
            naga::Statement::Return {
                value: Some(expression),
            },
            naga::Span::UNDEFINED,
        );
    }
    fn push_bare_return(&mut self) {
        self.push(
            naga::Statement::Return { value: None },
            naga::Span::UNDEFINED,
        );
    }
    fn push_store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    ) {
        self.push(
            naga::Statement::Store { pointer, value },
            naga::Span::UNDEFINED,
        );
    }
    fn push_if(
        &mut self,
        condition: naga::Handle<naga::Expression>,
        accept: naga::Block,
        reject: naga::Block,
    ) {
        self.push(
            naga::Statement::If {
                condition,
                accept,
                reject,
            },
            naga::Span::UNDEFINED,
        );
    }
    fn push_kill(&mut self) {
        self.push(naga::Statement::Kill, naga::Span::UNDEFINED);
    }
}

pub struct FunctionSignature {
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

    ($module:expr => fn $fn_name:tt ( $($arg_name:ident : $arg_ty:expr),* $(,)? ) $(-> $ret_ty:expr)?) => {{
        #[allow(unused_mut)]
        #[allow(unused_assignments)]
        let mut result: Option<naga::Handle<naga::Type>> = None;
        $(
            result = Some($ret_ty);
        )?

        let function_signature = $crate::FunctionSignature {
            name: $crate::declare_function!(@match_fn_name $fn_name),
            args: vec![
                $(
                    (stringify!{$arg_name}.to_owned(), $arg_ty),
                )*
            ],
            result,
        };

        let function_handle = $crate::ModuleExt::new_function($module, function_signature);

        let function = $module.functions.get_mut(function_handle.clone());

        let mut i = 0;
        (function_handle, $(
            function.expressions.append(naga::Expression::FunctionArgument({
                let _ = stringify!{$arg_name};
                let v = i;
                i += 1;
                v
            }), naga::Span::UNDEFINED),
        )*)
    }};
}

#[macro_export]
macro_rules! naga_expr {
    ($a:expr => $($expression:tt)*) => {{
        #[allow(unused)]
        let mut ctx = $crate::BlockContext::from($a);
        $crate::naga_expr!(@inner ctx => $($expression)*)
    }};

    ($a:expr, $b:expr => $($expression:tt)*) => {{
        #[allow(unused)]
        let mut ctx = $crate::BlockContext::from(($a, $b));
        $crate::naga_expr!(@inner ctx => $($expression)*)
    }};

    // Inner expressions (let bindings)
    (@inner_eat_to_semi $ctx:expr => let $var:ident = {$($eaten:tt)*} ; $($others:tt)*) => {{
        let $var = $crate::naga_expr!(@inner $ctx => $($eaten)*);
        $crate::naga_expr!(@inner $ctx => $($others)*)
    }};
    (@inner_eat_to_semi $ctx:expr => let $var:ident = {$($eaten:tt)*} $next:tt $($others:tt)*) => {
        $crate::naga_expr!(@inner_eat_to_semi $ctx => let $var = {$($eaten)* $next} $($others)*)
    };
    (@inner $ctx:expr => let $var:ident = $($others:tt)*) => {
        $crate::naga_expr!(@inner_eat_to_semi $ctx => let $var = {} $($others)*)
    };

    // Resizing
    (@inner $ctx:expr => bitcast<u32>($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Uint $($others)*)
    }};
    (@inner $ctx:expr => bitcast<i32>($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Sint $($others)*)
    }};
    (@inner $ctx:expr => bitcast<f32>($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Float $($others)*)
    }};
    (@inner $ctx:expr => u32($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Uint (4) $($others)*)
    }};
    (@inner $ctx:expr => i32($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Sint (4) $($others)*)
    }};
    (@inner $ctx:expr => f32($($value:tt)*) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => ($($value)*) as Float (4) $($others)*)
    }};
    (@inner $ctx:expr => $lhs:tt as $kind:tt ($bitcount:expr) $($others:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let handle = $ctx.append_expression(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: Some($bitcount) });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Casting
    (@inner $ctx:expr => $lhs:tt as $kind:tt $($others:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let handle = $ctx.append_expression(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Bin Ops
    (@inner $ctx:expr => $lhs:tt + $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Add, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt - $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Subtract, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt * $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Multiply, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt / $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Divide, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt % $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Modulo, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt >> $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::ShiftRight, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt << $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::ShiftLeft, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt | $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::InclusiveOr, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt ^ $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::ExclusiveOr, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt & $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::And, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt > $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Greater, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt >= $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::GreaterEqual, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt < $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Less, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt <= $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::LessEqual, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt == $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::Equal, left, right });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => $lhs:tt != $($rhs:tt)*) => {{
        let left = $crate::naga_expr!(@inner $ctx => $lhs);
        let right = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Binary { op: naga::BinaryOperator::NotEqual, left, right });
        $ctx.emit(handle)
    }};

    (@inner $ctx:expr => !$($value:tt)*) => {{
        let value = $crate::naga_expr!(@inner $ctx => $($value)*);
        let handle = $ctx.append_expression(naga::Expression::Unary { op: naga::UnaryOperator::LogicalNot, expr: value });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => ~$($value:tt)*) => {{
        let value = $crate::naga_expr!(@inner $ctx => $($value)*);
        let handle = $ctx.append_expression(naga::Expression::Unary { op: naga::UnaryOperator::BitwiseNot, expr: value });
        $ctx.emit(handle)
    }};
    (@inner $ctx:expr => -$($value:tt)*) => {{
        let value = $crate::naga_expr!(@inner $ctx => $($value)*);
        let handle = $ctx.append_expression(naga::Expression::Unary { op: naga::UnaryOperator::Negate, expr: value });
        $ctx.emit(handle)
    }};

    // Struct Ops
    (@inner $ctx:expr => $base:tt [const $index:expr ] $($others:tt)*) => {{
        let base = $crate::naga_expr!(@inner $ctx => $base);
        let handle = $ctx.append_expression(naga::Expression::AccessIndex{ base, index: $index });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Array Ops
    (@inner $ctx:expr => $base:tt [ $($index:tt)* ] $($others:tt)*) => {{
        let base = $crate::naga_expr!(@inner $ctx => $base);
        let index = $crate::naga_expr!(@inner $ctx => $($index)*);
        let handle = $ctx.append_expression(naga::Expression::Access { base, index });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Constants
    (@inner $ctx:expr => Bool($term:expr) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => Literal(naga::Literal::Bool($term)) $($others)*)
    }};
    (@inner $ctx:expr => I32($term:expr) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => Literal(naga::Literal::I32($term)) $($others)*)
    }};
    (@inner $ctx:expr => U32($term:expr) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => Literal(naga::Literal::U32($term)) $($others)*)
    }};
    (@inner $ctx:expr => F32($term:expr) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => Literal(naga::Literal::F32($term)) $($others)*)
    }};
    (@inner $ctx:expr => F64($term:expr) $($others:tt)*) => {{
        $crate::naga_expr!(@inner $ctx => Literal(naga::Literal::F64($term)) $($others)*)
    }};

    // Getting references to things
    (@inner $ctx:expr => Local($term:expr) $($others:tt)*) => {{
        let handle = $ctx.local_expr($term);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => Constant($term:expr) $($others:tt)*) => {{
        let handle = $ctx.constant_expr($term);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => Literal($term:expr) $($others:tt)*) => {{
        let handle = $ctx.literal_expr($term);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => Global($term:expr) $($others:tt)*) => {{
        let handle = $ctx.global_expr($term);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Deref
    (@inner $ctx:expr => Load($($pointer:tt)*) $($others:tt)*) => {{
        let pointer = $crate::naga_expr!(@inner $ctx => $($pointer)*);
        let handle = $ctx.append_expression(naga::Expression::Load { pointer });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Maths
    (@inner $ctx:expr => countLeadingZeros($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::CountLeadingZeros, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => exp2($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Exp2, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => abs($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Abs, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => ceil($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Ceil, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => floor($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Floor, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => trunc($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Trunc, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => round($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Round, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => sqrt($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Sqrt, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => sign($($arg:tt)*) $($others:tt)*) => {{
        let arg = $crate::naga_expr!(@inner $ctx => $($arg)*);
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Sign, arg, arg1: None, arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => min($($args:tt)*) $($others:tt)*) => {{
        let mut components = Vec::new();
        $crate::naga_expr!{@innerconstructor $ctx, components => $($args)* }
        let arg = components[0];
        let arg1 = components[1];
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Min, arg, arg1: Some(arg1), arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};
    (@inner $ctx:expr => max($($args:tt)*) $($others:tt)*) => {{
        let mut components = Vec::new();
        $crate::naga_expr!{@innerconstructor $ctx, components => $($args)* }
        let arg = components[0];
        let arg1 = components[1];
        let handle = $ctx.append_expression(naga::Expression::Math { fun: naga::MathFunction::Max, arg, arg1: Some(arg1), arg2: None, arg3: None });
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Inline if
    (@inner $ctx:expr => if ( $($condition:tt)* ) { $($lhs:tt)* } else { $($rhs:tt)* } ) => {{
        let condition = $crate::naga_expr!(@inner $ctx => $($condition)* );
        let accept = $crate::naga_expr!(@inner $ctx => $($lhs)*);
        let reject = $crate::naga_expr!(@inner $ctx => $($rhs)*);
        let handle = $ctx.append_expression(naga::Expression::Select { condition, accept, reject });
        $ctx.emit(handle)
    }};

    // Constructors
    (@innerconstructor $ctx:expr, $components:expr => $e1:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $ctx => $e1));
        $(naga_expr!(@innerconstructor $ctx, $components => $($others)*);)*
    }};
    (@innerconstructor $ctx:expr, $components:expr => $e1:tt $e2:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $ctx => $e1 $e2));
        $(naga_expr!(@innerconstructor $ctx, $components => $($others)*);)*
    }};
    (@innerconstructor $ctx:expr, $components:expr => $e1:tt $e2:tt $e3:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $ctx => $e1 $e2 $e3));
        $(naga_expr!(@innerconstructor $ctx, $components => $($others)*);)*
    }};
    (@innerconstructor $ctx:expr, $components:expr => $e1:tt $e2:tt $e3:tt $e4:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $ctx => $e1 $e2 $e3 $e4));
        $(naga_expr!(@innerconstructor $ctx, $components => $($others)*);)*
    }};
    (@innerconstructor $ctx:expr, $components:expr => $e1:tt $e2:tt $e3:tt $e4:tt $e5:tt $(, $($others:tt)*)?) => {{
        $components.push(naga_expr!(@inner $ctx => $e1 $e2 $e3 $e4 $e5));
        $(naga_expr!(@innerconstructor $ctx, $components => $($others)*);)*
    }};
    (@inner $ctx:expr => $ty:tt ( $($args:tt)* ) $($others:tt)*) => {{
        let mut components = Vec::new();
        $crate::naga_expr!{@innerconstructor $ctx, components => $($args)* }
        let handle = $ctx.append_expression(naga::Expression::Compose {ty: $ty, components});
        $ctx.emit(handle);
        $crate::naga_expr!(@inner $ctx => handle $($others)*)
    }};

    // Step braces
    (@inner $ctx:expr => ($($expression:tt)*) $($others:tt)*) => {{
        let lhs = $crate::naga_expr!(@inner $ctx => $($expression)*);
        $crate::naga_expr!(@inner $ctx => lhs $($others)*)
    }};

    // Arbitrary embeddings
    (@inner $ctx:expr => {$term:expr}) => { $term };
    (@inner $ctx:expr => $term:expr) => { $term };
}
