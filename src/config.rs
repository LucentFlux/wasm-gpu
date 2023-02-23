#[derive(Debug, Copy, Clone)]
pub struct Tuneables {
    /// If set to true, the translator will output f64 instructions. If false,
    /// a polyfill will be used
    pub hardware_supports_f64: bool,
    /// The size of the workgroups per invocation, with y and z being set to 1
    pub workgroup_size: u32,
}

impl Default for Tuneables {
    fn default() -> Self {
        Self {
            hardware_supports_f64: false,
            workgroup_size: 256,
        }
    }
}
