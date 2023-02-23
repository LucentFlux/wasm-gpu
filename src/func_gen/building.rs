//! Some functions and tools to streamline manually building naga functions from data
use super::WorkingFunction;

pub(crate) fn make_inner_func_result(ty: naga::Handle<naga::Type>) -> Option<naga::FunctionResult> {
    Some(naga::FunctionResult { ty, binding: None })
}

pub(crate) fn make_fn_return<'a, F: WorkingFunction<'a>>(
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

#[macro_export]
macro_rules! naga_fn_def {
    ($working:expr => fn $fn_name:tt ( $($arg_name:tt : $arg_ty:tt),* $(,)? ) $(-> $ret_ty:tt)?) => {{
        $working.get_fn_mut().name = Some(stringify!{$fn_name}.to_owned());
        $working.get_fn_mut().arguments = vec![
            $(
                naga::FunctionArgument { name: Some(stringify!{$arg_name}.to_owned()), ty: $arg_ty, binding: None }
            ),*
        ];
        $(
            $working.get_fn_mut().result =
            $crate::func_gen::building::make_inner_func_result($ret_ty);
        )*

        ($(
            $working.get_fn_mut().expressions.append(naga::Expression::FunctionArgument(${ignore(arg_name)} ${index()}), naga::Span::UNDEFINED),
        )*)
    }};
}
pub(crate) use naga_fn_def;

#[macro_export]
macro_rules! naga_expr {
    (@emit $working:expr => $handle:ident) => {
        $working.get_fn_mut().body
            .push(naga::Statement::Emit(naga::Range::new_from_bounds(
                $handle, $handle,
            )), naga::Span::UNDEFINED);
    };

    ($working:expr => ($($terms:tt)*)) => {
        naga_expr!($working => $($terms)*)
    };

    // Casting
    ($working:expr => $lhs:tt as $kind:tt) => {{
        let left = naga_expr!($working => $lhs);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::As { expr: left, kind: naga::ScalarKind::$kind, convert: None }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};

    // Bin Ops
    ($working:expr => $lhs:tt + $rhs:tt) => {{
        let left = naga_expr!($working => $lhs);
        let right = naga_expr!($working => $rhs);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Add, left, right }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};
    ($working:expr => $lhs:tt - $rhs:tt) => {{
        let left = naga_expr!($working => $lhs);
        let right = naga_expr!($working => $rhs);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Subtract, left, right }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};
    ($working:expr => $lhs:tt * $rhs:tt) => {{
        let left = naga_expr!($working => $lhs);
        let right = naga_expr!($working => $rhs);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Multiply, left, right }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};
    ($working:expr => $lhs:tt / $rhs:tt) => {{
        let left = naga_expr!($working => $lhs);
        let right = naga_expr!($working => $rhs);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::Binary { op: naga::BinaryOperator::Divide, left, right }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};

    // Struct Ops
    ($working:expr => $base:tt [const $index:expr ]) => {{
        let base = naga_expr!($working => $base);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::AccessIndex{ base, index: $index }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};

    // Array Ops
    ($working:expr => $base:tt [ $($index:tt)* ]) => {{
        let base = naga_expr!($working => $base);
        let index = naga_expr!($working => $($index)*);
        let handle = $working.get_fn_mut().expressions.append(naga::Expression::Access { base, index }, naga::Span::UNDEFINED);

        naga_expr!{@emit $working => handle}

        handle
    }};

    // Constants
    ($working:expr => I32($term:expr)) => {{
        let const_handle = $working.module.constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Sint($term as i64) },
        }, naga::Span::UNDEFINED);
        $working.get_fn_mut().expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED)
    }};
    ($working:expr => U32($term:expr)) => {{
        let const_handle = $working.module.constants.append(naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar { width: 4, value: naga::ScalarValue::Uint($term as u64) },
        }, naga::Span::UNDEFINED);
        $working.get_fn_mut().expressions.append(naga::Expression::Constant(const_handle), naga::Span::UNDEFINED)
    }};

    // Constructors
    ($working:expr => $type:tt ( $($element:tt),* $(,)? )) => {{
        let components = vec![
            $(
                naga_expr!($working => $element),
            )*
        ];
        let handle = $working.get_fn_mut().expressions.append(
            naga::Expression::Compose {ty: $type, components},
            naga::Span::UNDEFINED,
        );

        naga_expr!{@emit $working => handle}

        handle
    }};

    // Arbitrary embeddings
    ($working:expr => $term:ident) => { $term };
}

pub(crate) use naga_expr;
