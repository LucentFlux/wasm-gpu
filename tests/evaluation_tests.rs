//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation
use wasm_gpu_test_lib::test_parity;

macro_rules! do_test {
    ($test_name:ident( $($input:expr),* $(,)? )) => {
        paste::paste! {
            #[test]
            fn [< $test_name $(_ $input)* >]() {
                $test_name($($input),*)
            }
        }
    };
}

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

#[test]
fn pass_through_local_return_i32() {
    test_parity::<i32, i32>(
        r#"
        (module
            (func $f (param i32) (result i32) (local $l i32)
                (local.get 0)
                (local.set $l)
                (local.get $l)
            )
            (export "pass_through_local_i32" (func $f))
        )
        "#,
        "pass_through_local_i32",
        -10840,
    )
}

#[test]
fn pass_through_local_return_i64() {
    test_parity::<i64, i64>(
        r#"
        (module
            (func $f (param i64) (result i64) (local $l i64)
                (local.get 0)
                (local.set $l)
                (local.get $l)
            )
            (export "pass_through_local_i64" (func $f))
        )
        "#,
        "pass_through_local_i64",
        -9223372036854675604,
    )
}

#[test]
fn pass_through_local_return_f32() {
    test_parity::<f32, f32>(
        r#"
        (module
            (func $f (param f32) (result f32) (local $l f32)
                (local.get 0)
                (local.set $l)
                (local.get $l)
            )
            (export "pass_through_local_f32" (func $f))
        )
        "#,
        "pass_through_local_f32",
        1.0001001f32,
    )
}

#[test]
fn pass_return_through_local_f64() {
    test_parity::<f64, f64>(
        r#"
        (module
            (func $f (param f64) (result f64) (local $l f64)
                (local.get 0)
                (local.set $l)
                (local.get $l)
            )
            (export "pass_through_local_f64" (func $f))
        )
        "#,
        "pass_through_local_f64",
        1.000001000001f64,
    )
}

/*
#[test]
fn unreachable_traps() {
    test_parity::<(), ()>(
        r#"
        (module
            (func $f
                unreachable
            )
            (export "trap_unnreachable" (func $f))
        )
        "#,
        "trap_unnreachable",
        (),
    )
}*/

#[test]
fn add_5_i32() {
    test_parity::<i32, i32>(
        r#"
        (module
            (func $f (param i32) (result i32)
                (local.get 0)
                (i32.const 5)
                (i32.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        8192,
    )
}

#[test]
fn add_5_i64() {
    test_parity::<i64, i64>(
        r#"
        (module
            (func $f (param i64) (result i64)
                (local.get 0)
                (i64.const 5)
                (i64.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        -9223372036854675604,
    )
}

#[test]
fn add_5_f32() {
    test_parity::<f32, f32>(
        r#"
        (module
            (func $f (param f32) (result f32)
                (local.get 0)
                (f32.const 5)
                (f32.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        1.0001001f32,
    )
}

/*#[test]
fn add_5_f64() {
    test_parity::<f64, f64>(
        r#"
        (module
            (func $f (param f64) (result f64)
                (local.get 0)
                (f64.const 5)
                (f64.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        1.000001000001f64,
    )
}*/

#[test]
fn excess_return() {
    test_parity::<f32, f32>(
        r#"
        (module
            (func $f (param f32) (result f32)
                (local.get 0)
                (return)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        1.00021001f32,
    )
}

#[test]
fn early_return() {
    test_parity::<f32, f32>(
        r#"
        (module
            (func $f (param f32) (result f32)
                (f32.const 12.01)
                (return)
                (local.get 0)
                (return)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        1.00031001f32,
    )
}

#[test]
fn bare_break() {
    test_parity::<(), f32>(
        r#"
        (module
            (func $f (result f32)
                (f32.const 12.01)
                (br 0)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
}

fn br_if_taken(input: i32) {
    test_parity::<i32, f32>(
        r#"
        (module
            (func $f (param i32) (result f32)
                (f32.const 12.0)
                (local.get 0)
                (br_if 0)
                (f32.const 5.0)
                (f32.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        input,
    )
}

do_test!(br_if_taken(0));
do_test!(br_if_taken(1));
do_test!(br_if_taken(2));

fn nested_blocks_br_if(input: i32) {
    test_parity::<i32, i32>(
        r#"
        (module
            (func $f (param i32) (result i32)
                (local i32)
                (block
                    (block
                        (block
                            ;; x == 0
                            local.get 0
                            i32.eqz
                            br_if 0

                            ;; x == 1
                            local.get 0
                            i32.const 1
                            i32.eq
                            br_if 1

                            ;; else
                            i32.const 7
                            local.set 1
                            br 2
                        )
                        i32.const 42
                        local.set 1
                        br 1
                    )
                    i32.const 99
                    local.set 1
                )
                local.get 1
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        input,
    )
}

do_test!(nested_blocks_br_if(0));
do_test!(nested_blocks_br_if(1));
do_test!(nested_blocks_br_if(2));
do_test!(nested_blocks_br_if(3));

fn nested_if(input: i32) {
    test_parity::<i32, f32>(
        r#"
        (module
            (func $f (param i32) (result f32)
                ;; x == 0
                local.get 0
                i32.eqz
                (if (result f32)
                    (then
                        f32.const 12.0
                    ) 
                    (else
                        local.get 0
                        i32.const 1
                        i32.eq
                        ;; x == 1
                        (if (result f32)
                            (then
                                f32.const 12.5
                            )
                            (else
                                f32.const 13.01
                            )
                        )
                    )
                )
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        input,
    )
}

do_test!(nested_if(0));
do_test!(nested_if(1));
do_test!(nested_if(2));
do_test!(nested_if(3));
