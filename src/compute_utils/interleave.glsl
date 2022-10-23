#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

layout (constant_id = 0) const int STRIDE = 1;

layout(set = 0, binding = 0) buffer DataIn {
    uint data[];
} v_in;

layout(set = 0, binding = 1) buffer DataOut {
    uint data[];
} v_out;

layout(set = 0, binding = 2) buffer Vars {
    uint count;
} vars;

void clone_input_id(uint index) {
    uint input_index = index * STRIDE;
    uint output_index = index * STRIDE * vars.count;

    // Hope that these get placed in registers
    uint vals[STRIDE];
    for (uint offset = 0; offset < STRIDE; offset++) {
        vals[offset] = v_in.data[input_index + offset];
    }

    // Loop and write
    for (uint i_child = 0; i_child < vars.count; i_child++) {
        for (uint offset = 0; offset < STRIDE; offset++) {
            v_out.data[output_index + (i_child * STRIDE) + offset] = vals[offset];
        }
    }
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    clone_input_id(idx);
}
