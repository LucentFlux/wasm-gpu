use self::{std_fns::StdFnSet, std_tys::StdTySet};

mod std_fns;
mod std_tys;

pub(crate) struct StdObjects {
    pub(crate) fns: StdFnSet,
    pub(crate) tys: StdTySet,
}

impl StdObjects {
    pub(crate) fn new() -> Self {
        Self {
            fns: StdFnSet::new(),
            tys: StdTySet::new(),
        }
    }
}
