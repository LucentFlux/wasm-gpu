# WASM-GPU funcgen

Generates the naga functions that are used to invoke wasm instances on GPUs.

# TODO

- Implement type safe naga generation wrapper
- Implement better naga function definition language rather than hacky expressions, and wrap in a proc macro
- Populate std_objects lazily while generating wasm-shader
- Parse and populate at the same time
- Implement f64 polyfill
- Implement recursion using brain function

# Stretch goals

- No-panic