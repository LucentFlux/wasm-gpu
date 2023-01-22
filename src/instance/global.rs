pub mod builder;
pub mod immutable;
pub mod instance;

macro_rules! impl_global_get {
    ($v:vis async fn get<$T:ident>(&mut self, queue: &AsyncQueue, ptr: &$Ptr:ty) -> Result<Val, BufferAsyncError>) => {
        async fn get_val<$T, V: WasmTyVal>(&mut self, queue: &AsyncQueue, ptr: &$Ptr) -> Result<Val, BufferAsyncError> {
            Ok(self.get_typed::<$T, V>(queue, ptr).await?.to_val())
        }

        $v async fn get<$T>(&mut self, queue: &AsyncQueue, ptr: &$Ptr) -> Result<Val, BufferAsyncError> {
            assert!(
                self.cap_set.check(&ptr.cap),
                "global pointer was not valid for this instance"
            );

            match &ptr.content_type() {
                ValType::I32 => self.get_val::<$T, i32>(queue, ptr).await,
                ValType::I64 => self.get_val::<$T, i64>(queue, ptr).await,
                ValType::F32 => self.get_val::<$T, Ieee32>(queue, ptr).await,
                ValType::F64 => self.get_val::<$T, Ieee64>(queue, ptr).await,
                ValType::V128 => self.get_val::<$T, u128>(queue, ptr).await,
                ValType::FuncRef => self.get_val::<$T, FuncRef>(queue, ptr).await,
                ValType::ExternRef => self.get_val::<$T, ExternRef>(queue, ptr).await,
            }
        }
    };
}

use impl_global_get;

macro_rules! impl_global_push {
    ($v:vis async fn push<$T:ident>(&mut self, queue: &AsyncQueue, val: Val) -> $Ret:ty) => {
        $v async fn push<$T>(&mut self, queue: &AsyncQueue, val: Val) -> $Ret {
            match val {
                Val::I32(v) => self.push_typed(queue, v).await,
                Val::I64(v) => self.push_typed(queue, v).await,
                Val::F32(v) => self.push_typed(queue, v).await,
                Val::F64(v) => self.push_typed(queue, v).await,
                Val::V128(v) => self.push_typed(queue, v).await,
                Val::FuncRef(v) => self.push_typed(queue, v).await,
                Val::ExternRef(v) => self.push_typed(queue, v).await,
            }
        }
    };
}

use impl_global_push;
