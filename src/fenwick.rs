use fenwick::array::{prefix_sum, update};

#[derive(Clone)]
pub struct FenwickTree {
    data: Vec<usize>,
}

impl FenwickTree {
    pub fn new() {}

    pub fn update(&mut self, index: usize, delta: usize) {
        update(self.data.as_mut_slice(), index, delta);
    }

    pub fn prefix_sum(&self, index: usize) -> usize {
        if index == 0 {
            return 0;
        }
        prefix_sum(self.data.as_slice(), index - 1)
    }
}
