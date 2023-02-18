use self::{std_fns::StdFnSet, std_tys::StdTySet};

mod std_fns;
mod std_tys;

pub struct StdObjects {
    pub fns: StdFnSet,
    pub tys: StdTySet,
}

impl StdObjects {
    pub fn new() -> Self {
        Self {
            fns: StdFnSet::new(),
            tys: StdTySet::new(),
        }
    }
}
