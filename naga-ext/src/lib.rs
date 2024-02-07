#![feature(macro_metavar_expr)]

//! Provide a collection of shorthand and opinionated methods extending base naga objects.
mod sealed {
    pub trait ModuleSealed {}
    impl ModuleSealed for naga::Module {}
    pub trait FunctionsSealed {}
    impl FunctionsSealed for naga::Arena<naga::Function> {}
    pub trait ConstantsSealed {}
    impl ConstantsSealed for naga::Arena<naga::Constant> {}
    pub trait TypesSealed {}
    impl TypesSealed for naga::UniqueArena<naga::Type> {}
    pub trait LocalsSealed {}
    impl LocalsSealed for naga::Arena<naga::LocalVariable> {}
    pub trait ExpressionsSealed {}
    impl ExpressionsSealed for naga::Arena<naga::Expression> {}
    pub trait FunctionSealed {}
    impl FunctionSealed for naga::Function {}
    pub trait BlockSealed {}
    impl BlockSealed for naga::Block {}
}

pub trait ModuleExt: self::sealed::ModuleSealed {
    /// Shorthand for `module.functions.new_empty_function(name)`
    fn new_empty_function(&mut self, name: String) -> naga::Handle<naga::Function>;
    /// Shorthand for `module.functions.new_function(definition)`
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
    /// Shorthand for `module.functions.get_mut(handle)`
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

pub trait FunctionsExt: self::sealed::FunctionsSealed {
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

pub trait ConstantsExt: self::sealed::ConstantsSealed {
    fn append_anonymous(&mut self, v: naga::ConstantInner) -> naga::Handle<naga::Constant>;

    fn append_i32(&mut self, v: i32) -> naga::Handle<naga::Constant>;
    fn append_i64(&mut self, v: i64) -> naga::Handle<naga::Constant>;
    fn append_u32(&mut self, v: u32) -> naga::Handle<naga::Constant>;
    fn append_u64(&mut self, v: u64) -> naga::Handle<naga::Constant>;
    fn append_f32(&mut self, v: f32) -> naga::Handle<naga::Constant>;
    fn append_f64(&mut self, v: f64) -> naga::Handle<naga::Constant>;

    fn append_bool(&mut self, v: bool) -> naga::Handle<naga::Constant>;
}

impl ConstantsExt for naga::Arena<naga::Constant> {
    fn append_anonymous(&mut self, inner: naga::ConstantInner) -> naga::Handle<naga::Constant> {
        self.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner,
            },
            naga::Span::UNDEFINED,
        )
    }

    fn append_u32(&mut self, v: u32) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 4,
            value: naga::ScalarValue::Uint(v as u64),
        })
    }

    fn append_u64(&mut self, v: u64) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 8,
            value: naga::ScalarValue::Uint(v),
        })
    }

    fn append_f32(&mut self, v: f32) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 4,
            value: naga::ScalarValue::Float(v as f64),
        })
    }

    fn append_f64(&mut self, v: f64) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 8,
            value: naga::ScalarValue::Float(v),
        })
    }

    fn append_i32(&mut self, v: i32) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 4,
            value: naga::ScalarValue::Sint(v as i64),
        })
    }

    fn append_i64(&mut self, v: i64) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 8,
            value: naga::ScalarValue::Sint(v),
        })
    }

    fn append_bool(&mut self, v: bool) -> naga::Handle<naga::Constant> {
        self.append_anonymous(naga::ConstantInner::Scalar {
            width: 1,
            value: naga::ScalarValue::Bool(v),
        })
    }
}

pub trait TypesExt: self::sealed::TypesSealed {}

impl TypesExt for naga::UniqueArena<naga::Type> {}

pub trait FunctionExt: self::sealed::FunctionSealed {}

impl FunctionExt for naga::Function {}

pub trait LocalsExt: self::sealed::LocalsSealed {
    // Shorthand handle generation
    fn new_local(
        &mut self,
        name: impl Into<String>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
    ) -> naga::Handle<naga::LocalVariable>;
}
impl LocalsExt for naga::Arena<naga::LocalVariable> {
    fn new_local(
        &mut self,
        name: impl Into<String>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
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

pub trait ExpressionsExt: self::sealed::ExpressionsSealed {
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
}
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
}

pub trait BlockExt: self::sealed::BlockSealed {
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
        let mut result = None;
        $(
            result = Some($ret_ty);
        )?

        let function_signature = naga_ext::FunctionSignature {
            name: naga_ext::declare_function!(@match_fn_name $fn_name),
            args: vec![
                $(
                    (stringify!{$arg_name}.to_owned(), $arg_ty),
                )*
            ],
            result,
        };

        let function_handle = naga_ext::ModuleExt::new_function($module, function_signature);

        let function = $module.functions.get_mut(function_handle.clone());

        (function_handle, $(
            function.expressions.append(naga::Expression::FunctionArgument(${ignore(arg_name)} ${index()}), naga::Span::UNDEFINED),
        )*)
    }};
}

/// Something into which an expression/statement can be built
pub trait ShaderPart {
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    );
}

impl ShaderPart
    for (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    )
{
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        (self.0, self.1, self.2)
    }
}

impl<'a> ShaderPart for (&'a mut naga::Module, naga::Handle<naga::Function>) {
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        let function: &mut naga::Function = self.0.functions.get_mut(self.1);
        (
            &mut self.0.constants,
            &mut function.expressions,
            &mut function.body,
        )
    }
}

impl ShaderPart
    for (
        &mut naga::Module,
        naga::Handle<naga::Function>,
        &mut naga::Block,
    )
{
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        let function: &mut naga::Function = self.0.functions.get_mut(self.1);
        (
            &mut self.0.constants,
            &mut function.expressions,
            &mut self.2,
        )
    }
}

#[macro_export]
macro_rules! naga_expr {
    ($shader_part:expr => $($expression:tt)*) => {{
        #[allow(unused)]
        let (mut constants, mut expressions, mut block) = naga_ext::ShaderPart::parts($shader_part);
        naga_ext::naga_expr!(@inner constants, expressions, block => $($expression)*)
    }};

    ($a:expr, $b:expr => $($expression:tt)*) => {{
        // Force reborrow -- this is a hack
        let mut pair = (&mut *$a, $b);
        naga_ext::naga_expr!(&mut pair => $($expression)*)
    }};

    ($a:expr, $b:expr, $c:expr => $($expression:tt)*) => {{
        // Force reborrow -- this is a hack
        let mut pair = (&mut *$a, $b, &mut $c);
        naga_ext::naga_expr!(&mut pair => $($expression)*)
    }};

    (@emit $block:expr => $handle:ident) => {{
        $block
            .push(naga::Statement::Emit(naga::Range::new_from_bounds(
                $handle, $handle,
            )), naga::Span::UNDEFINED);
        $handle
    }};

    // Inner expressions (let bindings)
    (@inner_eat_to_semi $constants:expr, $expressions:expr, $block:expr => let $var:ident = {$($eaten:tt)*} ; $($others:tt)*) => {{
        let $var = naga_expr!(@inner $constants, $expressions, $block => $($eaten)*);
        naga_expr!(@inner $constants, $expressions, $block => $($others)*)
    }};
    (@inner_eat_to_semi $constants:expr, $expressions:expr, $block:expr => let $var:ident = {$($eaten:tt)*} $next:tt $($others:tt)*) => {
        naga_expr!(@inner_eat_to_semi $constants, $expressions, $block => let $var = {$($eaten)* $next} $($others)*)
    };
    (@inner $constants:expr, $expressions:expr, $block:expr => let $var:ident = $($others:tt)*) => {
        naga_expr!(@inner_eat_to_semi $constants, $expressions, $block => let $var = {} $($others)*)
    };

    // Resizing
    (@inner $constants:expr, $expressions:expr, $block:expr => bitcast<u32>($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Uint $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => bitcast<i32>($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Sint $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => bitcast<f32>($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Float $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => u32($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Uint (4) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => i32($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Sint (4) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => f32($($value:tt)*) $($others:tt)*) => {{
        naga_expr!(@inner $constants, $expressions, $block => ($($value)*) as Float (4) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt as $kind:tt ($bitcount:expr) $($others:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let handle = $expressions.append(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: Some($bitcount) }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Casting
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt as $kind:tt $($others:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let handle = $expressions.append(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Bin Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt + $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Add, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt - $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Subtract, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt * $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Multiply, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt / $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Divide, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt % $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Modulo, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt >> $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::ShiftRight, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt << $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::ShiftLeft, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt | $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::InclusiveOr, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt ^ $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::ExclusiveOr, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt & $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::And, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt > $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Greater, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt >= $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::GreaterEqual, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt < $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Less, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt <= $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::LessEqual, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt == $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Equal, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => $lhs:tt != $($rhs:tt)*) => {{
        let left = naga_expr!(@inner $constants, $expressions, $block => $lhs);
        let right = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::NotEqual, left, right }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};

    (@inner $constants:expr, $expressions:expr, $block:expr => !$($value:tt)*) => {{
        let value = naga_expr!(@inner $constants, $expressions, $block => $($value)*);
        let handle = $expressions.append(naga::Expression::Unary { op: naga::UnaryOperator::Not, expr: value }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => -$($value:tt)*) => {{
        let value = naga_expr!(@inner $constants, $expressions, $block => $($value)*);
        let handle = $expressions.append(naga::Expression::Unary { op: naga::UnaryOperator::Negate, expr: value }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
    }};

    // Struct Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $base:tt [const $index:expr ] $($others:tt)*) => {{
        let base = naga_expr!(@inner $constants, $expressions, $block => $base);
        let handle = $expressions.append(naga::Expression::AccessIndex{ base, index: $index }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Array Ops
    (@inner $constants:expr, $expressions:expr, $block:expr => $base:tt [ $($index:tt)* ] $($others:tt)*) => {{
        let base = naga_expr!(@inner $constants, $expressions, $block => $base);
        let index = naga_expr!(@inner $constants, $expressions, $block => $($index)*);
        let handle = $expressions.append(naga::Expression::Access { base, index }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Constants
    (@inner $constants:expr, $expressions:expr, $block:expr => Bool($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_bool($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => I32($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_i32($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => U32($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_u32($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => F32($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_f32($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => I64($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_i64($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => U64($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_u64($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => F64($term:expr) $($others:tt)*) => {{
        let const_handle = naga_ext::ConstantsExt::append_f64($constants, $term);
        naga_expr!(@inner $constants, $expressions, $block => Constant(const_handle) $($others)*)
    }};

    // Getting references to things
    (@inner $constants:expr, $expressions:expr, $block:expr => Local($term:expr) $($others:tt)*) => {{
        let handle = naga_ext::ExpressionsExt::append_local($expressions, $term);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => Constant($term:expr) $($others:tt)*) => {{
        let handle = naga_ext::ExpressionsExt::append_constant($expressions, $term);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => Global($term:expr) $($others:tt)*) => {{
        let handle = naga_ext::ExpressionsExt::append_global($expressions, $term);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Deref
    (@inner $constants:expr, $expressions:expr, $block:expr => Load($($pointer:tt)*) $($others:tt)*) => {{
        let pointer = naga_expr!(@inner $constants, $expressions, $block => $($pointer)*);
        let handle = $expressions.append(naga::Expression::Load { pointer }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Maths
    (@inner $constants:expr, $expressions:expr, $block:expr => countLeadingZeros($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::CountLeadingZeros, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => exp2($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Exp2, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => abs($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Abs, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => ceil($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Ceil, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => floor($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Floor, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => trunc($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Trunc, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => round($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Round, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => sqrt($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Sqrt, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => sign($($arg:tt)*) $($others:tt)*) => {{
        let arg = naga_expr!(@inner $constants, $expressions, $block => $($arg)*);
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Sign, arg, arg1: None, arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => min($($args:tt)*) $($others:tt)*) => {{
        let mut components = Vec::new();
        naga_expr!{@innerconstructor $constants, $expressions, $block, components => $($args)* }
        let arg = components[0];
        let arg1 = components[1];
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Min, arg, arg1: Some(arg1), arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};
    (@inner $constants:expr, $expressions:expr, $block:expr => max($($args:tt)*) $($others:tt)*) => {{
        let mut components = Vec::new();
        naga_expr!{@innerconstructor $constants, $expressions, $block, components => $($args)* }
        let arg = components[0];
        let arg1 = components[1];
        let handle = $expressions.append(naga::Expression::Math { fun: naga::MathFunction::Max, arg, arg1: Some(arg1), arg2: None, arg3: None }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Inline if
    (@inner $constants:expr, $expressions:expr, $block:expr => if ( $($condition:tt)* ) { $($lhs:tt)* } else { $($rhs:tt)* } ) => {{
        let condition = naga_expr!(@inner $constants, $expressions, $block => $($condition)* );
        let accept = naga_expr!(@inner $constants, $expressions, $block => $($lhs)*);
        let reject = naga_expr!(@inner $constants, $expressions, $block => $($rhs)*);
        let handle = $expressions.append(naga::Expression::Select { condition, accept, reject }, naga::Span::UNDEFINED);
        naga_ext::naga_expr!(@emit $block => handle)
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
        naga_ext::naga_expr!(@emit $block => handle);
        naga_expr!(@inner $constants, $expressions, $block => handle $($others)*)
    }};

    // Step braces
    (@inner $constants:expr, $expressions:expr, $block:expr => ($($expression:tt)*) $($others:tt)*) => {{
        let lhs = naga_ext::naga_expr!(@inner $constants, $expressions, $block => $($expression)*);
        naga_expr!(@inner $constants, $expressions, $block => lhs $($others)*)
    }};

    // Arbitrary embeddings
    (@inner $constants:expr, $expressions:expr, $block:expr => {$term:expr}) => { $term };
    (@inner $constants:expr, $expressions:expr, $block:expr => $term:expr) => { $term };
}
