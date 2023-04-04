use crate::func_gen::ActiveFunction;

pub(crate) fn make_fn_return<'a, F: ActiveFunction<'a>>(
    working: &mut F,
    statement: naga::Handle<naga::Expression>,
) {
    working.get_fn_mut().body.push(
        naga::Statement::Return {
            value: Some(statement),
        },
        naga::Span::UNDEFINED,
    );
}

mod sealed {
    pub trait ModuleSealed {}
    impl ModuleSealed for naga::Module {}
    pub trait FunctionSealed {}
    impl FunctionSealed for naga::Function {}
}

pub(crate) trait ModuleExt: self::sealed::ModuleSealed {
    fn new_function(&mut self, definition: FunctionSignature) -> naga::Handle<naga::Function>;
    /// Shorthand
    fn fn_mut(&mut self, handle: naga::Handle<naga::Function>) -> &mut naga::Function;
}

impl ModuleExt for naga::Module {
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

        let handle = self.functions.append(new_function, naga::Span::UNDEFINED);
        return handle;
    }

    fn fn_mut(&mut self, handle: naga::Handle<naga::Function>) -> &mut naga::Function {
        self.functions.get_mut(handle)
    }
}

pub(crate) trait FunctionExt: self::sealed::FunctionSealed {
    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression>;

    fn push_return(&mut self, expression: naga::Handle<naga::Expression>);
    fn push_store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    );
}
impl FunctionExt for naga::Function {
    fn append_global(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.expressions.append(
            naga::Expression::GlobalVariable(global),
            naga::Span::UNDEFINED,
        )
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
    ($working:expr => $($expression:tt)*) => {{
        let (module, function) = $working.get_active();
        $crate::naga_expr!(@inner module, function => $($expression)*)
    }};

    ($module:expr, $function:expr => $($expression:tt)*) => {{
        let function = $module.functions.get_mut($function.clone());
        $crate::naga_expr!(@inner $module, function => $($expression)*)
    }};

    (@emit $function:expr => $handle:ident) => {{
        $function.body
            .push(naga::Statement::Emit(naga::Range::new_from_bounds(
                $handle, $handle,
            )), naga::Span::UNDEFINED);
        $handle
    }};

    (@inner $module:expr, $function:expr => ($($expression:tt)*)) => {{
        $crate::naga_expr!(@inner $module, $function => $($expression)*)
    }};

    // Casting
    (@inner $module:expr, $function:expr => $lhs:tt as $kind:tt) => {{
        let left = naga_expr!(@inner $module, $function => $lhs);
        let handle = $function.expressions.append(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: None }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Bin Ops
    (@inner $module:expr, $function:expr => $lhs:tt + $rhs:tt) => {{
        let left = naga_expr!(@inner $module, $function => $lhs);
        let right = naga_expr!(@inner $module, $function => $rhs);
        let handle = $function.expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Add, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};
    (@inner $module:expr, $function:expr => $lhs:tt - $rhs:tt) => {{
        let left = naga_expr!(@inner $module, $function => $lhs);
        let right = naga_expr!(@inner $module, $function => $rhs);
        let handle = $function.expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Subtract, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};
    (@inner $module:expr, $function:expr => $lhs:tt * $rhs:tt) => {{
        let left = naga_expr!(@inner $module, $function => $lhs);
        let right = naga_expr!(@inner $module, $function => $rhs);
        let handle = $function.expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Multiply, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};
    (@inner $module:expr, $function:expr => $lhs:tt / $rhs:tt) => {{
        let left = naga_expr!(@inner $module, $function => $lhs);
        let right = naga_expr!(@inner $module, $function => $rhs);
        let handle = $function.expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Divide, left, right }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Struct Ops
    (@inner $module:expr, $function:expr => $base:tt [const $index:expr ]) => {{
        let base = naga_expr!(@inner $module, $function => $base);
        let handle = $function.expressions.append(naga::Expression::AccessIndex{ base, index: $index }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Array Ops
    (@inner $module:expr, $function:expr => $base:tt [ $($index:tt)* ]) => {{
        let base = naga_expr!(@inner $module, $function => $base);
        let index = naga_expr!(@inner $module, $function => $($index)*);
        let handle = $function.expressions.append(naga::Expression::Access { base, index }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Constants
    (@inner $module:expr, $function:expr => I32($term:expr)) => {{
        let const_handle = $module.constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Sint($term as i64) },
        }, naga::Span::UNDEFINED);
        $function.expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED)
    }};
    (@inner $module:expr, $function:expr => U32($term:expr)) => {{
        let const_handle = $module.constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Uint($term as u64) },
        }, naga::Span::UNDEFINED);
        $function.expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED)
    }};
    (@inner $module:expr, $function:expr => Global($term:expr)) => {{
        $function.expressions.append(naga::Expression::GlobalVariable($term), naga::Span::UNDEFINED)
    }};

    // Deref
    (@inner $module:expr, $function:expr => Load($pointer:tt)) => {{
        let pointer = naga_expr!(@inner $module, $function => $pointer);
        let handle = $function.expressions.append(naga::Expression::Load { pointer }, naga::Span::UNDEFINED);
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Constructors
    (@inner $module:expr, $function:expr => $ty:tt ( $($element:tt),* $(,)? )) => {{
        let components = vec![
            $(
                naga_expr!(@inner $module, $function => $element),
            )*
        ];
        let handle = $function.expressions.append(
            naga::Expression::Compose {ty: $ty, components},
            naga::Span::UNDEFINED,
        );
        $crate::naga_expr!(@emit $function => handle)
    }};

    // Arbitrary embeddings
    (@inner $module:expr, $function:expr => $term:ident) => { $term };
}
