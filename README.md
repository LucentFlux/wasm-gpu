# 0.1 Roadmap

- [x] Implement I32 instructions as in the specification
- [ ] Implement F32 instructions as in the specification
    - [x] Implement base operation emission
    - [x] Implement subnormal polyfill
    - [ ] Implement rounding correction polyfill
- [ ] Improve unit tests to distinguish between NaN types and Trap types
- [ ] Implement type safe naga generation wrapper (see typesafe branch of naga_ext)
- [ ] Implement better naga function definition language rather than hacky expressions, and wrap in a proc macro (and add to naga_ext and release both)
- [ ] Populate std_objects lazily while generating wasm-shader
- [ ] Parse and populate at the same time
- [ ] Implement i64 polyfill
- [ ] Implement f64 polyfill
- [ ] Implement recursion using brain function
- [x] Add support for suspending/recreating wasm modules
- [ ] Fully integrate testsuite
- [ ] Add fuzzer
- [ ] Improve optimisation with our own handrolled passes by looking at suboptimal output shaders

# Stretch goals

- [ ] No-panic