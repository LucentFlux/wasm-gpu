pub mod builder;
pub mod immutable;
pub mod instance;

macro_rules! impl_global_get {
    ($v:vis async fn get<$T:ident>(&mut self, ptr: &$Ptr:ty) -> Val) => {
        async fn get_val<$T, V: WasmTyVal>(&mut self, ptr: &$Ptr) -> Val {
            self.get_typed::<$T, V>(ptr).await.to_val()
        }

        $v async fn get<$T>(&mut self, ptr: &$Ptr) -> Val {
            assert!(
                self.cap_set.check(&ptr.cap),
                "global pointer was not valid for this instance"
            );

            match &ptr.content_type() {
                ValType::I32 => self.get_val::<$T, i32>(ptr).await,
                ValType::I64 => self.get_val::<$T, i64>(ptr).await,
                ValType::F32 => self.get_val::<$T, Ieee32>(ptr).await,
                ValType::F64 => self.get_val::<$T, Ieee64>(ptr).await,
                ValType::V128 => self.get_val::<$T, u128>(ptr).await,
                ValType::FuncRef => self.get_val::<$T, FuncRef>(ptr).await,
                ValType::ExternRef => self.get_val::<$T, ExternRef>(ptr).await,
            }
        }
    };
}

use impl_global_get;

macro_rules! impl_global_push {
    ($v:vis async fn push<$T:ident>(&mut self, val: Val) -> $Ret:ty) => {
        $v async fn push<$T>(&mut self, val: Val) -> $Ret {
            match val {
                Val::I32(v) => self.push_typed(v).await,
                Val::I64(v) => self.push_typed(v).await,
                Val::F32(v) => self.push_typed(v).await,
                Val::F64(v) => self.push_typed(v).await,
                Val::V128(v) => self.push_typed(v).await,
                Val::FuncRef(v) => self.push_typed(v).await,
                Val::ExternRef(v) => self.push_typed(v).await,
            }
        }
    };
}

use impl_global_push;
