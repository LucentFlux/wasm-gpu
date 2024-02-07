pub mod builder;
pub mod instance;

pub fn wasm_limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => m1 >= m2,
        (_, _) => false,
    };
}
