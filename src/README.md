# Design Doc

This module deals with representing web assembly functions as naga IR. This naga IR can then be transformed to SPIR-V, or GLSL or HLSL or any number of graphics backends. 

# Data representation

Single-word integers (`I32`s) and single-word floats (`F32`s) are stored as is.

Double-word integers (`I64`s) are stored as vec2s of u32s little-endian order, i.e. the least significant word at the first index of the vector. For signed operations the top bit of the second component must be interpreted as the sign.

If supported, double-word floats (`F64`s) are stored as is. If not,
double-word floats (`F64`s) are stored as vec3s of f32s, such that: if v = vec3(a, b, c) then the value stored in v is given by (a + b) * (2 ^ c). The double-word float is normalised if c = some integer i, a and b have the same sign, and the exponent of b is 24 less than the exponent of a.

Vectors (`V128`s) are stored as a vec4 of u32s in little-endian order. For vector operations they are reinterpreted, then operated on, then reinterpreted back. It is assumed that reinterpretation has no actual runtime cost.

# Call order

Unfortunately, SPIR-V (and by extension naga) have some requirements on functions that make this mapping difficult. Mainly, GPUs do not support recursion and the call graph of a shader must be a DAG.

To accommodate this, we do not convert always wasm functions to naga functions one-to-one. When a loop is found in the call graph, we instead simulate a stack in a main brain function, which calls out to portions of translated wasm code. These child functions do not themselves call other child functions; they instead modify the stack in a way that causes the brain function to call the other child functions. We call this construction a WASM-Machine.

This brain function must be the entry point for every invocation that wishes to recurse, so that any function call can call another function (without a direct call in the call graph) by returning down to the brain function. However most functions would suffer from the overhead of this construction -- especially on a GPU, where the fragmentation of a warp would lead to huge performance hits. Therefore we duplicate every function, and construct both a 'regular' and a 'stack machine' variant.

This duplication, combined with a total order on the functions calculated from the direct call graph, allows a compile-time cycle-free version of the module to be generated, where non-recursive code is translated as expected but recursive code can be called into.

```
         ┌─────────────┐      ┌──────────────────┐      ┌─────────────┐
         │             │      │                  │      │             │
         │  Function1  │      │                  ├─────►│  Function1  │
         │             │      │                  │      │             │
         └──────┬──────┘      │                  │      └────┬────────┘
Calling 'down'  │             │                  │           : ▲ Pseudo-calling through
the total order ▼             │                  │           ▼ : the brain method
         ┌─────────────┐      │                  │      ┌──────┴──────┐
         │             │      │                  │      │             │
         │  Function2  ├─────►│                  ├─────►│  Function2  │
         │             │      │                  │      │             │
         └─────────────┘      │                  │      └─────────────┘
                              │   Brain Method   │             ▲
                              │                  │             :
         ┌─────────────┐      │                  │      ┌──────┴──────┐
         │             │      │                  │      │             │
         │  Function3  ├─────►│                  ├─────►│  Function3  │
         │             │      │                  │      │             │
         └──────┬──────┘      │                  │      └────┬────────┘
                │             │                  │           : ▲
                ▼             │                  │           ▼ :
         ┌─────────────┐      │                  │      ┌──────┴──────┐
         │             │      │                  │      │             │
         │  Function4  ├─────►│                  ├─────►│  Function4  │
         │             │      │                  │      │             │
         └─────────────┘      └──────────────────┘      └─────────────┘
```

## Child functions

If the simulated stack is viewed as storing program counters, child functions can be seen as operations that can be performed by the emulated virtual machine. However, unlike a traditional virtual machine, the WASM-Machine generated only has to perform some limited number of actions, and so does not need to be general purpose. Instead, each child function is the maximal set of instructions that can be performed in sequence before ending in a branch or function call. This makes them similar to basic blocks, except that function calls cannot occur mid-chid, as a function call must be made by the brain function, requiring a return. 

Instead we split a basic block at a function call so that on a call, not only is the funtion to call pushed to the stack, the program counter to return to is replaced with the next basic block to call.

## ABI

The stack consists of 32-bit words, and grows upwards within the stack buffer. The stack pointer is a uint giving the first unused word above the stack. A stack frame consists of locals, followed by a BlockID (BIDs are like PCs but, as above, reference a portion of a basic block rather than a single instruction). Pseudo-functions are responsable for saving all of their registers, since each child block is not necesarily re-entered after a function call. Since we know the stack space used by each child block, it is the caller's responsability to reserve adeaquate space for all locals used by a function before placing the called function's BID above all arguments and returning to the brain function.

## Host functions

Only the first 2^31 values of BID space are reserved for BIDs of blocks generated from WASM. The other 2^31 values (with the MSB set) are reserved for host functions, so that exiting the shader (virtaul machine) early allows the engine to notice a non-empty stack (non-zero stack pointer) and execute some host function before restarting.

## Resuming

