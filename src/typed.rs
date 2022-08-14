use crate::for_each_function_signature;
use wasmtime::{WasmParams, WasmResults, WasmTy};

/// A parallel version of [`wasmtime::WasmParams`] where one of the inputs is marked as a range and
/// the rest are marked as constant. For example, (i32, &\[f32], i64) but not (i32, &\[f32], &\[i64]) or (i32,)
pub unsafe trait WaspParams: Send {
    #[doc(hidden)]
    type SingularType: WasmParams; // The type of the parameters without the slice type, in wasmtime space
}

unsafe impl<T> WaspParams for &[T]
where
    T: WasmTy,
{
    type SingularType = T;
}

#[cps::cps]
macro_rules! impl_wasm_params {
    (@iter $n:tt ($($t1:ident)*) () | ($($tall:ident)*)) => {};

    (@iter $n:tt ($($t1:ident)*) ($t:ident $($t2:ident)*) | ($($tall:ident)*)) =>
    {
        unsafe impl<$($tall),*> WaspParams for ($($t1,)* &[$t], $($t2),*)
        where
            T: WasmTy,
        {
            type SingularType = ($($tall),*);
        }

        impl_wasm_params!(@iter $n ($($t1)* $t) ($($t2)*) | ($($tall)*))
    };

    ($n:tt $($t:ident)*) =>
    {
        impl_wasm_params!(@iter $n () ($($t)*) | ($($t)*))
    }
}

for_each_function_signature!(impl_wasm_params);
