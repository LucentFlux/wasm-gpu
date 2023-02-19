#[derive(Debug, Copy, Clone)]
pub struct Tuneables {
    /// If set to true, the translator will output f64 instructions. If false,
    /// a polyfill will be used
    pub hardware_supports_f64: bool,
    /// The size of the workgroups per invocation
    pub workgroup_size: [u32; 3],
}

impl Default for Tuneables {
    fn default() -> Self {
        Self {
            hardware_supports_f64: false,
            workgroup_size: [256, 1, 1],
        }
    }
}
