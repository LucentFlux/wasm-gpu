@group(0)
@binding(0)
var<storage, read> v_in: array<u32>;

@group(0)
@binding(1)
var<storage, write> v_out: array<u32>;

@id(0) override stride: u32; // Number of u32s per element

struct Vars {
    count: u32,  // Number of elements
};

@group(0)
@binding(2)
var<storage, read> vars: Vars;

fn clone_input_id(index: u32) {
    var input_index: u32 = index * stride;
    var output_index: u32 = index * stride * vars.count;

    // Hope that these get placed in registers
    var vals: array<u32, stride>;
    for (var offset: u32 = 0; offset < stride; offset++) {
        var read_val: u32 = v_in[input_index + offset];
        vals[offset] = read_val;
    }

    // Loop and write
    for (var i_child: u32 = 0; i_child < vars.count; i_child++) {
        for (var offset: u32 = 0; offset < stride; offset++) {
            v_out[output_index + (i_child * stride) + offset] = vals[offset];
        }
    }
}

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    clone_input_id(global_id.x);
}