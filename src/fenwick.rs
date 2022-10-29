use fenwick::array::{prefix_sum, update};

#[derive(Clone)]
pub struct FenwickTree {
    data: Vec<usize>,
}

impl FenwickTree {
    pub fn new(lengths: impl ExactSizeIterator<Item = usize>) -> Self {
        let mut new = Self {
            data: vec![0usize; lengths.len()],
        };

        for (i, len) in lengths.enumerate() {
            new.update(i, len);
        }

        new
    }

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
