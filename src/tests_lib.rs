#![cfg(test)]

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

pub async fn get_backend() -> (MemorySystem, AsyncQueue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: true,
            compatible_surface: None,
        })
        .await
        .expect("could not aquire a test adapter");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_webgl2_defaults(),
            },
            None,
        )
        .await
        .expect("");

    let (device, queue) = wgpu_async::wrap_wgpu(device, queue);

    let conf = BufferRingConfig {
        // Minimal memory footprint for tests
        chunk_size: 1024,
        total_transfer_buffers: 2,
    };
    let memory_system = MemorySystem::new(device, conf);

    return (memory_system, queue);
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
