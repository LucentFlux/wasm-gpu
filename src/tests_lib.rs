#![cfg(test)]

use crate::backend::lazy::buffer_ring::BufferRingConfig;
use crate::wasp;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

pub async fn get_backend() -> wasp::VulkanoBackend {
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();
    let conf = wasp::VulkanoBackendConfig {
        buffer_ring: BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 2 * 1024,
        },
    };
    return wasp::VulkanoBackend::new(conf).await;
}

#[macro_export]
macro_rules! block_test {
    ($value:expr, $name:ident) => {
        paste! (
            #[test]
            fn [<$name _ $value>]() {
                Runtime::new().unwrap().block_on($name($value));
            }
        );
    }
}

pub fn gen_test_data(size: usize, seed: u32) -> Vec<u8> {
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
