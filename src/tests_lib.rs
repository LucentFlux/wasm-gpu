#![cfg(test)]

use crate::backend::lazy::buffer_ring::BufferRingConfig;
use crate::wasp;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

pub async fn get_backend() -> wasp::WgpuBackend {
    let conf = wasp::WgpuBackendConfig {
        buffer_ring: BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 2 * 1024,
        },
        backends: wgpu::Backends::all(),
        allowed_features: Default::default(),
    };
    return wasp::WgpuBackend::new(conf, None)
        .await
        .expect("couldn't create default test backend");
}

#[macro_export]
macro_rules! block_test {
    ($value:expr, $name:ident) => {
        paste::paste! (
            #[test]
            fn [<$name _ $value>]() {
                tokio::runtime::Runtime::new().unwrap().block_on($name($value));
            }
        );
    }
}

pub fn gen_test_data(size: usize, seed: u32) -> Vec<u8> {
    let seed = u32::wrapping_add(
        u32::wrapping_mul(seed, 65),
        u32::wrapping_mul(size as u32, 33),
    );

    let mut seed_2 = [0u8; 32];
    seed_2[0] = (seed) as u8;
    seed_2[1] = (seed >> 8) as u8;
    seed_2[2] = (seed >> 16) as u8;
    seed_2[3] = (seed >> 24) as u8;
    let mut rng = StdRng::from_seed(seed_2);

    let mut res = Vec::new();
    res.resize(size, 0);

    rng.fill_bytes(res.as_mut_slice());

    return res;
}

pub fn gen_test_memory_string(size: usize, seed: u32) -> (Vec<u8>, String) {
    let expected_data = gen_test_data(size, seed);

    let mut data_string = "".to_owned();
    for byte in expected_data.iter() {
        data_string += format!("\\{:02x?}", byte).as_str();
    }

    (expected_data, data_string)
}
