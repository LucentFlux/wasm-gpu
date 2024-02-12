use std::sync::atomic::AtomicUsize;

use naga_ext::{block_context::Test, naga_expr, BlockContext, TypesExt};

use crate::active_function::locals::FnLocal;

/// Used to generate unique IDs for each block
static BLOCK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// The webassembly `br` instruction has the ability to pierce through multiple layers of blocks at once.
/// To track this in our shader code, we assign an 'is_branching' boolean at each block layer, which
/// is used to check (on exit from a child block) whether the child is requesting that the branch continues
/// down the chain of blocks.
///
/// This is excessive, and we could optimise this system to only include propogation variables where required,
/// but this reduces the simplicity of our code and may introduce bugs. Instead, we trust the optimising compiler
/// of both spirv-tools and the driver to remove excess, leaving us to focus on correctness.
pub(super) struct BlockLabel {
    block_id: usize,
    inner: FnLocal,
    false_expr: naga::Handle<naga::Expression>,
    true_expr: naga::Handle<naga::Expression>,
}

impl BlockLabel {
    pub(super) fn id(&self) -> usize {
        self.block_id
    }

    pub(super) fn set(&self, ctx: &mut BlockContext<'_>) {
        let label_ptr = self.inner.expression;
        ctx.store(label_ptr, self.true_expr);
    }

    pub(super) fn unset(&self, ctx: &mut BlockContext<'_>) {
        let label_ptr = self.inner.expression;
        ctx.store(label_ptr, self.false_expr);
    }

    pub(super) fn if_is_set<'a>(&self, ctx: &'a mut BlockContext<'_>) -> Test<'a> {
        let label_ptr = self.inner.expression;
        let label_value = naga_expr!(ctx => Load(label_ptr));
        ctx.test(label_value)
    }
}

/// Generates a set of labels (local booleans) used for jumping through many scopes at once. See [`BlockLabel`].
#[derive(Clone, Copy)]
pub(super) struct BlockLabelGen {
    /// Held for initialising block labels
    bool_ty: naga::Handle<naga::Type>,
    false_expr: naga::Handle<naga::Expression>,
    true_expr: naga::Handle<naga::Expression>,
}

impl BlockLabelGen {
    pub(super) fn new(ctx: &mut BlockContext<'_>) -> Self {
        Self {
            bool_ty: ctx.types.insert_bool(),
            false_expr: ctx.literal_expr_from(false),
            true_expr: ctx.literal_expr_from(true),
        }
    }

    pub(super) fn get_label(&self, ctx: &mut BlockContext<'_>) -> BlockLabel {
        let block_id = BLOCK_COUNT.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        BlockLabel {
            block_id,
            inner: FnLocal::append_to(
                format!("branching_escape_flag_{}", block_id),
                ctx,
                self.bool_ty,
                Some(self.false_expr),
            ),
            false_expr: self.false_expr,
            true_expr: self.true_expr,
        }
    }
}
