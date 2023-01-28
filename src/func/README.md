# Design Doc - TODO: Implement this

This module deals with representing web assembly functions as naga IR. This naga IR can then be transformed to SPIR-V, or GLSL or HLSL or any number of graphics backends. 

Unfortunately, SPIR-V (and by extension naga) have some requirements on functions that make this mapping difficult. Mainly, GPUs do not support recursion and the call graph of a shader must be a DAG.

To accommodate this, we do not convert always wasm functions to naga functions one-to-one. When a loop is found in the call graph, we instead simulate a stack in a main brain function, which calls out to portions of translated wasm code. These child functions do not themselves call other child functions; they instead modify the stack in a way that causes the brain function to call the other child functions. We call this construction, for some co-dependent wasm functions, the functions' WASM-Machine.

## Child functions

If the simulated stack is viewed as storing program counters, child functions can be seen as operations that can be performed by the emulated virtual machine. However, unlike a traditional virtual machine, the WASM-Machine generated only has to perform some limited number of actions, and so does not need to be general purpose. Instead, each child function is the maximal set of instructions that can be performed in sequence before ending in a branch or function call. This makes them similar to basic blocks, except that function calls cannot occur mid-chid, as a function call must be made by the brain function, requiring a return. 

Instead we split a basic block at a function call so that on a call, not only is the funtion to call pushed to the stack, the program counter to return to is replaced with the next basic block to call.

## ABI

The stack consists of 32-bit words, and grows upwards within the stack buffer. The stack pointer is a uint giving the first unused word above the stack. A stack frame consists of locals, followed by a BlockID (BIDs are like PCs but, as above, reference a portion of a basic block rather than a single instruction). Pseudo-functions are responsable for saving all of their registers, since each child block is not necesarily re-entered after a function call. Since we know the stack space used by each child block, it is the caller's responsability to reserve adeaquate space for all locals used by a function before placing the called function's BID above all arguments and returning to the brain function.

## Host functions

Only the first 2^31 values of BID space are reserved for BIDs of blocks generated from WASM. The other 2^31 values (with the MSB set) are reserved for host functions, so that exiting the shader (virtaul machine) early allows the engine to notice a non-empty stack (non-zero stack pointer) and execute some host function before restarting.