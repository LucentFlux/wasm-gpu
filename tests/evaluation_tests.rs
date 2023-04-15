//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation
use wasm_gpu_test_lib::test_parity;

#[test]
fn bare_return_i32() {
    test_parity::<(), i32>(
        r#"
        (module
            (func $f (result i32)
                (i32.const 42)
            )
            (export "life_universe_and_everything" (func $f))
        )
        "#,
        "life_universe_and_everything",
        (),
    )
}

#[test]
fn bare_return_i64() {
    // 2 ^ 63 - 2
    test_parity::<(), i64>(
        r#"
        (module
            (func $f (result i64)
                (i64.const 9223372036854775805)
            )
            (export "some_big_number" (func $f))
        )
        "#,
        "some_big_number",
        (),
    )
}

#[test]
fn bare_return_f32() {
    test_parity::<(), f32>(
        r#"
        (module
            (func $f (result f32)
                (f32.const 19.000001)
            )
            (export "some_floaty_number" (func $f))
        )
        "#,
        "some_floaty_number",
        (),
    )
}

#[test]
fn bare_return_f64() {
    test_parity::<(), f64>(
        r#"
        (module
            (func $f (result f64)
                (f64.const 1900.000001)
            )
            (export "some_floatier_number" (func $f))
        )
        "#,
        "some_floatier_number",
        (),
    )
}

#[test]
fn pass_return_i32() {
    test_parity::<i32, i32>(
        r#"
        (module
            (func $f (param i32) (result i32)
                local.get 0
            )
            (export "pass_i32" (func $f))
        )
        "#,
        "pass_i32",
        -1084,
    )
}

#[test]
fn pass_return_i64() {
    test_parity::<i64, i64>(
        r#"
        (module
            (func $f (param i64) (result i64)
                local.get 0
            )
            (export "pass_i64" (func $f))
        )
        "#,
        "pass_i64",
        -9223372036854675804,
    )
}

#[test]
fn pass_return_f32() {
    test_parity::<f32, f32>(
        r#"
        (module
            (func $f (param f32) (result f32)
                local.get 0
            )
            (export "pass_f32" (func $f))
        )
        "#,
        "pass_f32",
        1.0000001f32,
    )
}

#[test]
fn pass_return_f64() {
    test_parity::<f64, f64>(
        r#"
        (module
            (func $f (param f64) (result f64)
                local.get 0
            )
            (export "pass_f64" (func $f))
        )
        "#,
        "pass_f64",
        1.000000000001f64,
    )
}
