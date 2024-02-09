use sealed::sealed;

#[sealed]
/// Any literal in Rust which can be converted into a [`naga::Literal`]
pub trait IntoLiteral {
    fn into_literal(self) -> naga::Literal;
}

#[sealed]
impl IntoLiteral for i32 {
    fn into_literal(self) -> naga::Literal {
        naga::Literal::I32(self)
    }
}

#[sealed]
impl IntoLiteral for u32 {
    fn into_literal(self) -> naga::Literal {
        naga::Literal::U32(self)
    }
}

#[sealed]
impl IntoLiteral for f32 {
    fn into_literal(self) -> naga::Literal {
        naga::Literal::F32(self)
    }
}

#[sealed]
impl IntoLiteral for f64 {
    fn into_literal(self) -> naga::Literal {
        naga::Literal::F64(self)
    }
}

#[sealed]
impl IntoLiteral for bool {
    fn into_literal(self) -> naga::Literal {
        naga::Literal::Bool(self)
    }
}
