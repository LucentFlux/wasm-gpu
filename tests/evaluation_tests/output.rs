use wasm_gpu_test_lib::test_parity;

#[test]
fn bare_return_i32() {
    test_parity::<(), i32, _>(
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
    test_parity::<(), i64, _>(
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
    test_parity::<(), Ieee32, _>(
        r#"
        (module
            (func $f (result f32)
                (f32.const 18.0000004)
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
    test_parity::<(), Ieee64, _>(
        r#"
        (module
            (func $f (result f64)
                (f64.const 1900.0000000000000004)
            )
            (export "some_floatier_number" (func $f))
        )
        "#,
        "some_floatier_number",
        (),
    )
}
