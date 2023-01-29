use std::collections::HashMap;

use crate::{
    instance::func::FuncsInstance,
    module::operation::{MVPOperator, OperatorByProposal, TailCallOperator},
    FuncRef, UntypedFuncPtr,
};
use petgraph::{graph::NodeIndex, Graph};

use super::{FuncInstance, FuncUnit};

pub struct CallGraph<T> {
    calls: Graph<UntypedFuncPtr<T>, ()>,
}

impl<T> CallGraph<T> {
    fn add_local_function(
        calls: &mut Graph<UntypedFuncPtr<T>, ()>,
        nodes: &HashMap<FuncRef, NodeIndex>,
        src_node: &NodeIndex,
        function: &FuncInstance<T>,
    ) {
        for operator in &function.func_data.operators {
            let dst_ref = match operator {
                OperatorByProposal::MVP(MVPOperator::Call { function_index })
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

    pub fn calculate(functions: &FuncsInstance<T>) -> Self {
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

            if let FuncUnit::LocalFunction(function) = functions.get(function_ptr) {
                // Host functions don't call functions
                Self::add_local_function(&mut calls, &nodes, src_node, function);
            }
        }

        // We don't need to add indirect calls since indirection is implemented by dropping down into the brain method anyway,
        // which prevents cycles in the call graph.

        Self { calls }
    }
}
