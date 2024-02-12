/// A linked stack is like a linked list, but where each node is immutably borrowed to push the next value.
/// This essentially 'freezes' the nodes in the stack until the nodes above it are popped via drops.
pub(crate) struct LinkedStack<'a, T> {
    value: T,
    parent: Option<&'a LinkedStack<'a, T>>,
}

impl<'a, T> LinkedStack<'a, T> {
    pub(crate) fn new(value: T) -> Self {
        LinkedStack {
            value,
            parent: None,
        }
    }
    pub(crate) fn push<'b>(&'b self, value: T) -> LinkedStack<'b, T> {
        LinkedStack {
            value,
            parent: Some(self),
        }
    }

    /// Borrows the top element.
    pub(crate) fn peek(&self) -> &T {
        &self.value
    }

    /// Gets the `n`th parent. An index of 0 gets the top of the stack
    pub(crate) fn peek_nth(&self, index: usize) -> Option<&T> {
        let mut node = self;

        for _ in 0..index {
            node = node.parent?;
        }

        return Some(&node.value);
    }

    /// Gets all nodes until the `n - 1`th parent. An index of 0 gets no nodes.
    pub(crate) fn peek_n<'b>(&'b self, len: usize) -> PeekedLinkedStack<'b, T> {
        PeekedLinkedStack {
            node: self,
            remaining: len,
        }
    }

    pub(crate) fn pop(self) -> T {
        self.value
    }
}

pub(crate) struct PeekedLinkedStack<'a, T> {
    node: &'a LinkedStack<'a, T>,
    remaining: usize,
}

impl<'a, T> Iterator for PeekedLinkedStack<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let value = &self.node.value;

        match self.node.parent {
            Some(node) => {
                self.node = node;
            }
            None => {
                self.remaining = 0;
            }
        }

        return Some(value);
    }
}
