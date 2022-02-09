#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint  width;
    uint  height;
} params;

layout(set = 0, binding = 0) buffer readonly Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer Sink { uint8_t data[]; } sink;

vec3 get_pixel(uvec2 pos) {
    uint idx = 3 * (params.width * pos.y + pos.x);
    uint8_t r = source.data[idx + 0];
    uint8_t g = source.data[idx + 1];
    uint8_t b = source.data[idx + 2];

    return vec3(r, g, b);
}

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy * 2;
    if (pos.x >= params.width || pos.y >= params.height) return;

    vec3 orig_rgb = (
        get_pixel(pos + uvec2(0, 0)) +
        get_pixel(pos + uvec2(0, 1)) +
        get_pixel(pos + uvec2(1, 0)) +
        get_pixel(pos + uvec2(1, 1))
    );
    //     get_pixel(pos + uvec2(1, 1)) +
    //     get_pixel(pos + uvec2(1, 2)) +
    //     get_pixel(pos + uvec2(2, 0)) +
    //     get_pixel(pos + uvec2(2, 1)) +
    //     get_pixel(pos + uvec2(2, 2))
    // ) / 9.0 * 4.0;

    // vec3 orig_rgb = get_pixel(pos);
    uvec3 quantized = uvec3(1024.0) - uvec3(round(orig_rgb));

    uint margin = params.width / 10;

    uvec3 goal_pos_x = uvec3(vec3(pos.x / 3.2) + (vec3(params.width) * vec3(0.01, 1.0/3.0 + 0.01, 2.0/3.0 + 0.01)));
    uvec3 target_base_idx = 3 * (params.width * quantized + goal_pos_x);

    // uvec3 target_base_idx = 3 * (params.width * quantized + pos.x);

    int new_value_r = sink.data[target_base_idx.r + 0] + sink.data[target_base_idx.r + 1] + 35;
    sink.data[target_base_idx.r + 0] = uint8_t(min(new_value_r, 255));
    sink.data[target_base_idx.r + 1] = uint8_t(min(max(0, new_value_r - 255), 255));
    sink.data[target_base_idx.r + 2] = uint8_t(min(max(0, new_value_r - 255), 255));

    int new_value_g = sink.data[target_base_idx.g + 0] + sink.data[target_base_idx.g + 1] + 35;
    sink.data[target_base_idx.g + 1] = uint8_t(min(new_value_g, 255));
    sink.data[target_base_idx.g + 0] = uint8_t(min(max(0, new_value_g - 255), 255));
    sink.data[target_base_idx.g + 2] = uint8_t(min(max(0, new_value_g - 255), 255));

    int new_value_b = sink.data[target_base_idx.b + 0] + sink.data[target_base_idx.b + 2] + 35;
    sink.data[target_base_idx.b + 2] = uint8_t(min(new_value_b, 255));
    sink.data[target_base_idx.b + 1] = uint8_t(min(max(0, new_value_b - 255), 255));
    sink.data[target_base_idx.b + 0] = uint8_t(min(max(0, new_value_b - 255), 255));

    // sink.data[target_base_idx.g + 1] += uint8_t(4.0);
    // sink.data[target_base_idx.b + 2] += uint8_t(4.0);
}
