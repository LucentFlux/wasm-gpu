use crate::{into_literal::IntoLiteral, LocalsExt};

/// A module and context into which an expression can be built using the [`naga_expr`] macro in this library.
pub struct BlockContext<'a> {
    // Module level
    pub types: &'a mut naga::UniqueArena<naga::Type>,
    pub constants: &'a mut naga::Arena<naga::Constant>,
    pub const_expressions: &'a mut naga::Arena<naga::Expression>,

    // Function level
    pub expressions: &'a mut naga::Arena<naga::Expression>,
    pub locals: &'a mut naga::Arena<naga::LocalVariable>,

    // Block level
    pub block: &'a mut naga::Block,
}

/// Reborrowing
impl<'a: 'b, 'b> From<&'b mut BlockContext<'a>> for BlockContext<'b> {
    fn from(value: &'b mut BlockContext<'a>) -> Self {
        Self {
            types: &mut *value.types,
            constants: &mut *value.constants,
            const_expressions: &mut *value.const_expressions,
            expressions: &mut *value.expressions,
            locals: &mut *value.locals,
            block: &mut *value.block,
        }
    }
}

/// Root function
impl<'a> From<(&'a mut naga::Module, naga::Handle<naga::Function>)> for BlockContext<'a> {
    fn from((module, function): (&'a mut naga::Module, naga::Handle<naga::Function>)) -> Self {
        let function = module.functions.get_mut(function);
        Self {
            types: &mut module.types,
            constants: &mut module.constants,
            const_expressions: &mut module.const_expressions,
            expressions: &mut function.expressions,
            locals: &mut function.local_variables,
            block: &mut function.body,
        }
    }
}

impl<'a> BlockContext<'a> {
    fn push_emit(&mut self, handle: naga::Handle<naga::Expression>) {
        // If the last statement was an emit, append this one to that one's range
        if let Some(naga::Statement::Emit(range)) = self.block.last_mut() {
            if let Some((start, end)) = range.first_and_last() {
                if end.index() + 1 == handle.index() {
                    *range = naga::Range::new_from_bounds(start, handle);
                    return;
                }
            }
        }

        self.block.push(
            naga::Statement::Emit(naga::Range::new_from_bounds(handle, handle)),
            naga::Span::UNDEFINED,
        );
    }

    /// Appends an expression, emitting it immediately if it needs to be emitted.
    #[inline(always)]
    pub fn append_expr(&mut self, expression: naga::Expression) -> naga::Handle<naga::Expression> {
        let shoult_emit = match expression {
            naga::Expression::Literal(_)
            | naga::Expression::Constant(_)
            | naga::Expression::ZeroValue(_)
            | naga::Expression::FunctionArgument(_)
            | naga::Expression::LocalVariable(_)
            | naga::Expression::GlobalVariable(_)
            | naga::Expression::CallResult(_)
            | naga::Expression::AtomicResult { .. }
            | naga::Expression::RayQueryProceedResult => false,

            naga::Expression::Compose { .. }
            | naga::Expression::Access { .. }
            | naga::Expression::AccessIndex { .. }
            | naga::Expression::Splat { .. }
            | naga::Expression::Swizzle { .. }
            | naga::Expression::Load { .. }
            | naga::Expression::ImageSample { .. }
            | naga::Expression::ImageLoad { .. }
            | naga::Expression::ImageQuery { .. }
            | naga::Expression::Unary { .. }
            | naga::Expression::Binary { .. }
            | naga::Expression::Select { .. }
            | naga::Expression::Derivative { .. }
            | naga::Expression::Relational { .. }
            | naga::Expression::Math { .. }
            | naga::Expression::As { .. }
            | naga::Expression::WorkGroupUniformLoadResult { .. }
            | naga::Expression::ArrayLength(_)
            | naga::Expression::RayQueryGetIntersection { .. } => true,
        };

        let handle = self.expressions.append(expression, naga::Span::UNDEFINED);

        if shoult_emit {
            self.push_emit(handle);
        }

        return handle;
    }

    /// Creates a new local variable in the function scope. The init expression, if present, must be an expression
    /// which can be evaluated at compile time, such as a literal.
    #[inline(always)]
    pub fn new_local(
        &mut self,
        name: impl Into<String>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::LocalVariable> {
        self.locals.new_local(name, ty, init)
    }

    #[inline(always)]
    pub fn local_expr(
        &mut self,
        local: naga::Handle<naga::LocalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.append_expr(naga::Expression::LocalVariable(local))
    }
    #[inline(always)]
    pub fn literal_expr(&mut self, literal: naga::Literal) -> naga::Handle<naga::Expression> {
        self.append_expr(naga::Expression::Literal(literal))
    }
    #[inline(always)]
    pub fn literal_expr_from(
        &mut self,
        literal: impl IntoLiteral,
    ) -> naga::Handle<naga::Expression> {
        self.literal_expr(literal.into_literal())
    }
    #[inline(always)]
    pub fn constant_expr(
        &mut self,
        constant: naga::Handle<naga::Constant>,
    ) -> naga::Handle<naga::Expression> {
        self.append_expr(naga::Expression::Constant(constant))
    }
    #[inline(always)]
    pub fn global_expr(
        &mut self,
        global: naga::Handle<naga::GlobalVariable>,
    ) -> naga::Handle<naga::Expression> {
        self.append_expr(naga::Expression::GlobalVariable(global))
    }

    /// Builds a [`naga::Statement::If`] using the given condition.
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    /// let bool_ty = module.types.insert_bool();
    /// let u32_ty = module.types.insert_u32();
    /// let (function, arg1) = naga_ext::declare_function! {&mut module =>
    ///     fn foo(arg1: bool_ty) -> u32_ty
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, function));
    /// ctx.test(arg1)
    ///     .then(|mut ctx| {
    ///         let const_1u32 = ctx.append_literal_from(1u32);
    ///         ctx.result(const_1u32)
    ///     })
    ///     .otherwise(|mut ctx| {
    ///         let const_0u32 = ctx.append_literal_from(0u32);
    ///         ctx.result(const_0u32)
    ///     });
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    #[inline(always)]
    pub fn test<'b>(&'b mut self, condition: naga::Handle<naga::Expression>) -> Test<'b> {
        Test {
            ctx: self.into(),
            condition,
        }
    }

    /// Builds a [`naga::Statement::Store`] using the given pointer and value.
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    /// let u32_ty = module.types.insert_u32();
    /// let (function, arg1) = naga_ext::declare_function! {&mut module =>
    ///     fn foo(arg1: u32_ty)
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, function));
    /// let local1 = ctx.new_local("my_local", u32_ty, None);
    /// let local_ptr = ctx.append_local(local1);
    /// ctx.store(local_ptr, arg1);
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    #[inline(always)]
    pub fn store(
        &mut self,
        pointer: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    ) {
        self.block.push(
            naga::Statement::Store { pointer, value },
            naga::Span::UNDEFINED,
        )
    }

    /// Calls a function with [`naga::Statement::Call`], placing the result in an expression which is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    /// let u32_ty = module.types.insert_u32();
    ///
    /// // Function to be called
    /// let (fn_foo, _) = naga_ext::declare_function! {&mut module =>
    ///     fn foo() -> u32_ty
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, fn_foo));
    /// let res = ctx.literal_expr_from(10u32);
    /// ctx.result(res);
    ///
    /// let (fn_bar, _) = naga_ext::declare_function! {&mut module =>
    ///     fn bar() -> u32_ty
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, fn_bar));
    /// let res = ctx.call_get_return(fn_foo, vec![]);
    /// ctx.result(res);
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    ///
    /// The above code results in the following shader:
    ///
    /// ```wgsl
    /// fn foo() -> u32 {
    ///     return 10;
    /// }
    /// fn bar() -> u32 {
    ///     return foo();
    /// }
    /// ```
    #[inline(always)]
    pub fn call_get_return(
        &mut self,
        function: naga::Handle<naga::Function>,
        arguments: Vec<naga::Handle<naga::Expression>>,
    ) -> naga::Handle<naga::Expression> {
        let result = self.append_expr(naga::Expression::CallResult(function));
        self.block.push(
            naga::Statement::Call {
                function,
                arguments,
                result: Some(result),
            },
            naga::Span::UNDEFINED,
        );
        return result;
    }

    /// Calls a function with [`naga::Statement::Call`].
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    ///
    /// // Function to be called
    /// let (fn_foo,) = naga_ext::declare_function! {&mut module =>
    ///     fn foo()
    /// };
    ///
    /// let (fn_bar,) = naga_ext::declare_function! {&mut module =>
    ///     fn bar()
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, fn_bar));
    /// ctx.call_void(fn_foo, vec![]);
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    ///
    /// The above code results in the following shader:
    ///
    /// ```wgsl
    /// fn foo() { }
    /// fn bar() {
    ///     foo();
    /// }
    /// ```
    #[inline(always)]
    pub fn call_void(
        &mut self,
        function: naga::Handle<naga::Function>,
        arguments: Vec<naga::Handle<naga::Expression>>,
    ) {
        self.block.push(
            naga::Statement::Call {
                function,
                arguments,
                result: None,
            },
            naga::Span::UNDEFINED,
        );
    }

    /// Builds a [`naga::Statement::Return`] using the given value.
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    /// let bool_ty = module.types.insert_bool();
    /// let u32_ty = module.types.insert_u32();
    /// let (function, arg1) = naga_ext::declare_function! {&mut module =>
    ///     fn foo(arg1: bool_ty) -> u32_ty
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, function));
    /// ctx.test(arg1)
    ///     .then(|mut ctx| {
    ///         let const_1u32 = ctx.append_literal_from(1u32);
    ///         ctx.result(const_1u32)
    ///     })
    ///     .otherwise(|mut ctx| {
    ///         let const_0u32 = ctx.append_literal_from(0u32);
    ///         ctx.result(const_0u32)
    ///     });
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    #[inline(always)]
    pub fn result(self, value: naga::Handle<naga::Expression>) {
        self.block.push(
            naga::Statement::Return { value: Some(value) },
            naga::Span::UNDEFINED,
        )
    }

    /// Builds a [`naga::Statement::Return`] with no return value.
    ///
    /// # Example
    ///
    /// ```
    /// # use naga_ext::*;
    /// let mut module = naga::Module::default();
    /// let (function,) = naga_ext::declare_function! {&mut module =>
    ///     fn foo()
    /// };
    /// let mut ctx = naga_ext::BlockContext::from((&mut module, function));
    /// ctx.void_return();
    /// # naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&mut module).unwrap();
    /// ```
    #[inline(always)]
    pub fn void_return(self) {
        self.block.push(
            naga::Statement::Return { value: None },
            naga::Span::UNDEFINED,
        )
    }
}

/// Built by a call to [`BlockContext::test`], and must be consumed by a call to [`Test::then`].
#[must_use]
pub struct Test<'a> {
    ctx: BlockContext<'a>,
    condition: naga::Handle<naga::Expression>,
}

impl<'a> Test<'a> {
    /// Populates a block to be run if the test condition is true.
    #[inline]
    pub fn then(mut self, f: impl FnOnce(BlockContext<'_>)) -> TestThen<'a> {
        let condition_index = self.ctx.block.len();

        let mut accept_block = naga::Block::new();
        let accept_ctx = BlockContext {
            block: &mut accept_block,
            ..(&mut self.ctx).into()
        };
        f(accept_ctx);

        self.ctx.block.push(
            naga::Statement::If {
                condition: self.condition,
                accept: accept_block,
                reject: naga::Block::new(),
            },
            naga::Span::UNDEFINED,
        );

        TestThen {
            ctx: self.ctx,
            condition_index,
        }
    }
}

/// Built by a call to [`Test::then`], allowing an `else` block to be inserted with [`TestThen::otherwise`].
/// See [`BlockContext::test`] for details.
pub struct TestThen<'a> {
    ctx: BlockContext<'a>,
    condition_index: usize,
}

impl<'a> TestThen<'a> {
    /// Populates a block to be run if the test condition is false.
    #[inline]
    pub fn otherwise(self, f: impl FnOnce(BlockContext<'_>)) {
        if let naga::Statement::If { reject, .. } = &mut self.ctx.block[self.condition_index] {
            let reject_ctx = BlockContext {
                block: reject,
                ..self.ctx
            };
            f(reject_ctx);
        } else {
            panic!(
                "context block was changed between a call to `Test::then` and `TestThen::otherwise"
            )
        }
    }
}
