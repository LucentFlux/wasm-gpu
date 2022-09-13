use itertools::Itertools;

/// Utility for taking vectors of Results and unwrapping them into vectors of values in one go if none of them are errors
pub trait PanicOnAny {
    type Item;
    type Iterator;

    fn unwrap_all(self) -> Self::Iterator;
    fn expect_all(self, msg: &str) -> Self::Iterator;
}

impl<E: std::fmt::Debug, T> PanicOnAny for Vec<Result<T, E>> {
    type Item = T;
    type Iterator = Vec<T>;

    fn unwrap_all(self) -> Self::Iterator {
        self.into_iter().map(|r| r.unwrap()).collect_vec()
    }

    fn expect_all(self, msg: &str) -> Self::Iterator {
        self.into_iter().map(|r| r.expect(msg)).collect_vec()
    }
}
