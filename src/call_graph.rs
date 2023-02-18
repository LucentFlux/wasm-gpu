use std::collections::HashMap;

use crate::{
    instance::func::FuncsInstance,
    module::operation::{ControlFlowOperator, OperatorByProposal, TailCallOperator},
    FuncRef, UntypedFuncPtr,
};
use itertools::Itertools;
use petgraph::{graph::NodeIndex, Graph};

use super::{FuncInstance, FuncUnit};

pub struct CallGraph {
    calls: Graph<UntypedFuncPtr, ()>,
}

impl CallGraph {
    fn add_local_function(
        calls: &mut Graph<UntypedFuncPtr, ()>,
        nodes: &HashMap<FuncRef, NodeIndex>,
        src_node: &NodeIndex,
        function: &FuncInstance,
    ) {
        for operator in &function.func_data.operators {
            let dst_ref = match operator {
                OperatorByProposal::ControlFlow(ControlFlowOperator::Call { function_index })
                | OperatorByProposal::TailCall(TailCallOperator::ReturnCall { function_index }) => {
                    function
                        .accessible()
                        .func_index_lookup
                        .get(
                            usize::try_from(*function_index)
                                .expect("16 bit architectures unsupported"),
                        )
                        .expect("an OoB function reference should be caught by validation")
                        .to_func_ref()
                }
                _ => continue, // Not a call
            };

            let dest_node = nodes
                .get(&dst_ref)
                .expect("every function was inserted into nodes");
            calls.add_edge(src_node.clone(), dest_node.clone(), ());
        }
    }

    pub fn calculate(functions: &FuncsInstance) -> Self {
        let mut calls = Graph::new();

        // Add all nodes
        let mut nodes = HashMap::new();
        let all_ptrs = functions.all_ptrs();
        for function_ptr in &all_ptrs {
            let node = calls.add_node(function_ptr.clone());
            nodes.insert(function_ptr.to_func_ref(), node);
        }

        // Add direct calls
        for function_ptr in &all_ptrs {
            let src_ref = function_ptr.to_func_ref();
            let src_node = nodes
                .get(&src_ref)
                .expect("every function was inserted into nodes");

            let FuncUnit::LocalFunction(function) = functions.get(function_ptr);
            Self::add_local_function(&mut calls, &nodes, src_node, function);
        }

        // We don't need to add indirect calls since indirection is implemented by dropping down into the brain method anyway,
        // which prevents cycles in the call graph.

        Self { calls }
    }

    fn get_externals(calls: &Graph<UntypedFuncPtr, ()>) -> Vec<NodeIndex> {
        calls
            .externals(petgraph::Direction::Incoming)
            .sorted()
            .rev()
            .collect_vec()
    }

    pub fn to_call_order(mut self) -> CallOrder {
        let mut call_order = Vec::new();

        while self.calls.node_count() != 0 {
            // Keep looping while there are externals
            let mut externals = Self::get_externals(&self.calls);
            while externals.len() != 0 {
                for external in externals {
                    let next_func = self
                        .calls
                        .remove_node(external)
                        .expect("nodes are removed in reverse order so indices are valid");
                    call_order.push(next_func);
                }

                externals = Self::get_externals(&self.calls);
            }

            // If there are no externals, take the node with the highest number of outgoing connections to try to make an external
            let max_connected = self
                .calls
                .node_indices()
                .max_by_key(|node| self.calls.edges(*node).count());
            if let Some(max_node) = max_connected {
                let next_func = self
                    .calls
                    .remove_node(max_node)
                    .expect("node pointer was just got");
                call_order.push(next_func);
            } else {
                assert!(self.calls.node_count() == 0); // Don't loop forever
            }
        }

        CallOrder::new(call_order)
    }
}

pub struct CallOrder {
    //ASSERT for all x, y: order[lookup[x.to_func_ref()]] == x && lookup[order[y].to_func_ref()] == y
    order: Vec<UntypedFuncPtr>,
    lookup: HashMap<FuncRef, usize>,
}

impl CallOrder {
    fn new(order: Vec<UntypedFuncPtr>) -> Self {
        let mut lookup = HashMap::new();

        for (i, ptr) in order.iter().enumerate() {
            lookup.insert(ptr.to_func_ref(), i);
        }

        Self { order, lookup }
    }

    pub fn get_in_order(&self) -> &Vec<UntypedFuncPtr> {
        &self.order
    }
}
