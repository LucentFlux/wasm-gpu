//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation
use wasm_gpu_test_lib::{test_parity, test_parity_set};

macro_rules! do_test {
    ($test_name:ident( $($input:expr),* $(,)? )) => {
        paste::paste! {
            #[tokio::test]
            async fn [< $test_name $(_ $input)* >]() {
                $test_name($($input),*).await
            }
        }
    };
}

#[tokio::test]
async fn bare_return_i32() {
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
    .await
}

#[tokio::test]
async fn bare_return_i64() {
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
    .await
}

#[tokio::test]
async fn bare_return_f32() {
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
    .await
}

#[tokio::test]
async fn bare_return_f64() {
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
    .await
}

#[tokio::test]
async fn pass_return_i32() {
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
    .await
}

#[tokio::test]
async fn pass_return_i64() {
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
    .await
}

#[tokio::test]
async fn pass_return_f32() {
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
    .await
}

#[tokio::test]
async fn pass_return_f64() {
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
    .await
}

#[tokio::test]
async fn pass_through_local_return_i32() {
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
    .await
}

#[tokio::test]
async fn pass_through_local_return_i64() {
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
    .await
}

#[tokio::test]
async fn pass_through_local_return_f32() {
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
    .await
}

#[tokio::test]
async fn pass_return_through_local_f64() {
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
    .await
}

#[tokio::test]
async fn unreachable_traps() {
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
    .await
}

#[tokio::test]
async fn get_i32_via_local() {
    test_parity::<(), i32>(
        r#"
        (module
            (func $f (result i32)
                (local $i i32)
                (i32.const 5)
                (local.set $i)
                (local.get $i)
                (local.get $i)
                (i32.add)
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

#[tokio::test]
async fn add_5_i32() {
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
    .await
}

#[tokio::test]
async fn add_5_i64() {
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
    .await
}

#[tokio::test]
async fn add_5_f32() {
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
    .await
}

/*#[tokio::test]
async fn add_5_f64() {
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

#[tokio::test]
async fn excess_return() {
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
    .await
}

#[tokio::test]
async fn early_return() {
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
    .await
}

#[tokio::test]
async fn bare_break() {
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
    .await
}

async fn br_if_taken(input: i32) {
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
    .await
}

do_test!(br_if_taken(0));
do_test!(br_if_taken(1));
do_test!(br_if_taken(2));

async fn nested_blocks_br_if(input: i32) {
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
    .await
}

do_test!(nested_blocks_br_if(0));
do_test!(nested_blocks_br_if(1));
do_test!(nested_blocks_br_if(2));
do_test!(nested_blocks_br_if(3));

async fn nested_if(input: i32) {
    test_parity::<i32, f32>(
        r#"
        (module
            (func $f (param i32) (result f32)
                f32.const 1.1
                ;; x <= 1
                local.get 0
                i32.const 1
                i32.le_s
                (if (param f32) (result f32)
                    (then
                        f32.const 12.0
                        f32.add
                        ;; x == 0
                        local.get 0
                        i32.eqz
                        (if (param f32) (result f32)
                            (then
                                f32.const 0.2
                                f32.add
                            )
                        )
                    )
                )
                ;; x <= 2
                local.get 0
                i32.const 2
                i32.le_s
                (if (param f32) (result f32)
                    (then
                        f32.const 3.5
                        f32.add
                    )
                )
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        input,
    )
    .await
}

do_test!(nested_if(0));
do_test!(nested_if(1));
do_test!(nested_if(2));
do_test!(nested_if(3));

async fn nested_if_then(input: i32) {
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
    .await
}

do_test!(nested_if_then(0));
do_test!(nested_if_then(1));
do_test!(nested_if_then(2));
do_test!(nested_if_then(3));

#[tokio::test]
async fn for_loop_break_from_inside() {
    test_parity::<(), i32>(
        r#"
        (module
            (func $f (result i32)
                (local $i i32) 
                (local.set $i (i32.const 1))                        ;; i = 1
                loop $LOOP
                    (i32.le_s (local.get $i) (i32.const 10))        ;; i <= 10
                    if
                        (local.set $i                               ;; i = i + 1
                            (i32.add (local.get $i) (i32.const 1)))
                        br $LOOP
                    end
                end
                local.get $i
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

#[tokio::test]
async fn for_loop_break_to_outside() {
    test_parity::<(), i32>(
        r#"
        (module
            (func $f (result i32)
                (local $i i32) 
                (local.set $i (i32.const 1))                        ;; i = 1
                (block
                    loop $LOOP
                        (i32.gt_s (local.get $i) (i32.const 7))         ;; i > 7
                        br_if 1
                            
                        (local.set $i                                   ;; i = i + 5
                            (i32.add (local.get $i) (i32.const 5)))
                        br $LOOP
                    end
                )
                local.get $i
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

#[tokio::test]
async fn for_loop_conditionally_dont_break_to_outside() {
    test_parity::<(), i32>(
        r#"
        (module
            (func $f (result i32)
                (local $i i32)

                ;; i = 1
                i32.const 1
                local.set $i

                (block $OUTER
                    (loop $LOOP
                        ;; i = i + 3
                        local.get $i
                        i32.const 3
                        i32.add
                        local.set $i

                        ;; i < 7
                        local.get $i
                        i32.const 7
                        i32.lt_s     
                        br_if $LOOP

                        br $OUTER
                    )
                )
                local.get $i
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

async fn read_memory_back(address: i32) {
    test_parity::<i32, i32>(
        r#"
            (module
                (memory (data "1029374529"))
                (func $f (param i32) (result i32)
                    local.get 0
                    i32.load
                )
                (export "foi" (func $f))
            )
        "#,
        "foi",
        address,
    )
    .await
}

do_test!(read_memory_back(0));
/*do_test!(read_memory_back(1));
do_test!(read_memory_back(2));
do_test!(read_memory_back(3));*/
do_test!(read_memory_back(4));
do_test!(read_memory_back(8));

async fn write_then_read_memory_back(address: i32) {
    test_parity::<i32, i32>(
        r#"
            (module
                (memory (data "00000000"))
                (func $f (param i32) (result i32)
                    local.get 0
                    i32.const 12345
                    i32.store
                    i32.const 0
                    i32.load
                )
                (export "foi" (func $f))
            )
        "#,
        "foi",
        address,
    )
    .await
}

do_test!(write_then_read_memory_back(0));
/*do_test!(write_then_read_memory_back(1));
do_test!(write_then_read_memory_back(2));
do_test!(write_then_read_memory_back(3));*/
do_test!(write_then_read_memory_back(4));
do_test!(write_then_read_memory_back(8));

#[tokio::test]
async fn trap_out_of_loop() {
    test_parity::<(), ()>(
        r#"
        (module
            (func $f
                (local i32)
                (local.set 0 (i32.const 100))
                loop $LOOP
                    (i32.div_s (i32.const 5) (local.get 0))
                    drop
                        
                    (local.set 0 (i32.sub (local.get 0) (i32.const 1)))

                    br $LOOP
                end
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

#[tokio::test]
async fn double_trap_only_gives_first() {
    test_parity::<(), ()>(
        r#"
        (module
            (func $f
                (i32.div_s (i32.const 5) (i32.const 0))
                drop
                (i32.div_s (i32.const 0x80000000) (i32.const -1))
                drop
            )
            (export "foi" (func $f))
        )
        "#,
        "foi",
        (),
    )
    .await
}

async fn mandelbrot(locs: Vec<(f32, f32)>) {
    test_parity_set::<_, f32>(
        r#"
            (module
                (func $f (param $x_0 f32) (param $y_0 f32) (param $max_iterations i32) (result f32)
                    (local $a f32)
                    (local $b f32)
                    (local $iterations i32)

                    ;; a = 0.0
                    f32.const 0.0
                    local.set $a
                    ;; b = 0.0
                    f32.const 0.0
                    local.set $b

                    ;; iterations = -1
                    i32.const -1
                    local.set $iterations

                    (loop $inner
                        ;; a_new = a * a - b * b + x0
                        local.get $a
                        local.get $a
                        f32.mul

                        local.get $b
                        local.get $b
                        f32.mul

                        f32.sub

                        local.get $x_0
                        f32.add

                        ;; b_new = 2.0 * a * b + y0
                        f32.const 2.0
                        local.get $a
                        f32.mul
                        local.get $b
                        f32.mul

                        local.get $y_0
                        f32.add

                        ;; a = a_new; b = b_new
                        local.set $b
                        local.set $a

                        ;; iterations += 1
                        local.get $iterations
                        i32.const 1
                        i32.add
                        local.set $iterations

                        ;; loop while iterations < max_iterations && a * a + b * b <= 4.0
                        local.get $iterations
                        local.get $max_iterations
                        i32.lt_s

                        local.get $a
                        local.get $a
                        f32.mul

                        local.get $b
                        local.get $b
                        f32.mul

                        f32.add

                        f32.const 4.0

                        f32.le

                        i32.and

                        br_if $inner
                    )

                    local.get $iterations
                    f32.convert_i32_s
                    local.get $max_iterations
                    f32.convert_i32_s
                    f32.div
                )
                (export "foi" (func $f))
            )
            "#,
        "foi",
        locs.into_iter().map(|(x, y)| (x, y, 1024)).collect(),
    )
    .await
}

#[tokio::test]
async fn mandelbrot_grid() {
    const SIZE: i32 = 1024;

    let locs = (-SIZE..SIZE)
        .flat_map(|x| {
            let x = x as f32 / SIZE as f32;
            let x = x / 2.0;
            (-SIZE..SIZE).map(move |y| {
                let y = y as f32 / SIZE as f32;
                let y = y / 2.0;

                (x, y)
            })
        })
        .collect();
    mandelbrot(locs).await
}
