use itertools::Itertools;
use wasm_spirv::{wasp, Config, Extern};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // wgpu setup
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

    // wasm setup
    let module_wat = r#"
    (module
      (type $t0 (func (param i32) (result i32)))
      (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
        get_local $p0
        i32.const 1
        i32.add))
    "#;

    let spirv_backend = wasp::WgpuBackend { device, queue };
    let engine = wasp::Engine::new(spirv_backend, Config::default());

    let module = wasp::Module::new(&engine, module_wat.as_bytes())?;

    let imports: Vec<Extern> = vec![];

    let instance = wasp::Instance::new(&engine, &module, imports.as_slice()).await?;

    let parallel_add_one_func = instance.get_typed_func::<i32, i32>("add_one")?;

    // Evaluate
    let parallel_result = parallel_add_one_func.call([1, 6, 4, 2, 7]).await;
    let parallel_result = parallel_result
        .into_iter()
        .enumerate()
        .map(|(i, v)| v.expect(format!("got error: {}", i).as_str()))
        .collect_vec();
    assert_eq!(vec![2, 7, 5, 3, 8], parallel_result);

    return Ok(());
}
