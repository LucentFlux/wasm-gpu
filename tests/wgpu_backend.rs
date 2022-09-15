use paste::paste;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use tokio::runtime::Runtime;
use wasm_spirv::{
    wasp, Backend, BufferRingConfig, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock, WgpuBackend,
    WgpuBackendConfig,
};

macro_rules! backend_buffer_test {
    ($value:expr, $name:ident) => {
        paste! (
            #[test]
            fn [<$name _ $value>]() {
                Runtime::new().unwrap().block_on($name($value));
            }
        );
    }
}

macro_rules! backend_buffer_tests {
    ($($value:expr,)*) => {
    $(
        backend_buffer_test!($value, test_get_unmapped_len);
        backend_buffer_test!($value, test_get_mapped_len);
        backend_buffer_test!($value, test_upload_download);
        backend_buffer_test!($value, test_create_mapped_download);
    )*
    };
}

backend_buffer_tests!(
    0, 1, 7, 8, 9, 1023, 1024, 1025, 1048575, //(1024 * 1024 - 1),
    1048576, //(1024 * 1024),
    1048577, //(1024 * 1024 + 1),
);

async fn get_backend() -> wasp::WgpuBackend {
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
    let conf = WgpuBackendConfig {
        buffer_ring_config: BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 128 * 1024,
            buffer_size: 1024,
        },
    };
    return wasp::WgpuBackend::new(device, queue, conf);
}

fn gen_test_data(size: usize, seed: u32) -> Vec<u8> {
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

#[inline(never)]
async fn test_get_unmapped_len(size: usize) {
    let mut backend = get_backend().await;

    let memory = backend.create_device_memory_block(size, None);

    assert_eq!(memory.len().await, size);
}

#[inline(never)]
async fn test_get_mapped_len(size: usize) {
    let mut backend = get_backend().await;

    let memory = backend.create_device_memory_block(size, None);
    let memory = memory.move_to_main_memory().await;

    assert_eq!(memory.len().await, size);
}

#[inline(never)]
async fn test_create_mapped_download(size: usize) {
    let mut backend = get_backend().await;

    let expected_data = gen_test_data(size, (size * 33) as u32);

    let memory = backend.create_device_memory_block(size, Some(expected_data.as_slice()));

    // Read
    let mut memory = memory.move_to_main_memory().await;
    let slice = memory.as_slice(..).await.expect("could not map memory");
    let data_result = Vec::from(slice);

    assert_eq!(data_result, expected_data);
}

#[inline(never)]
async fn test_upload_download(size: usize) {
    let mut backend = get_backend().await;

    let memory = backend.create_device_memory_block(size, None);
    let mut memory = memory.move_to_main_memory().await;
    let slice = memory.as_slice(..).await.expect("could not map memory");

    // Write some data
    let expected_data = gen_test_data(size, size as u32);
    slice.copy_from_slice(expected_data.as_slice());

    // Unmap and Remap
    let memory = memory.move_to_device_memory().await;
    let mut memory = memory.move_to_main_memory().await;
    let slice = memory.as_slice(..).await.expect("could not re-map memory");
    let data_result = Vec::from(slice);

    assert_eq!(data_result, expected_data);
}
