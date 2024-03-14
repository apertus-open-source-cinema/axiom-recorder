#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint width;
    uint height;

// these are actual coordinates of the first red pixel (unlike everywhere else)
    uint first_red_x;
    uint first_red_y;
} params;

layout(set = 0, binding = 0) buffer readonly Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint8_t data[]; } sink;

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy;
    if (pos.x >= params.width || pos.y >= params.height) return;

    uvec2 base_pos = pos * 2;

    uint red_first_x = params.first_red_x;
    uint red_first_y = params.first_red_y;
    sink.data[(pos.y * params.width + pos.x) * 3 + 0] = source.data[(base_pos.x + red_first_x) + (base_pos.y + red_first_y) * params.width * 2];
    sink.data[(pos.y * params.width + pos.x) * 3 + 2] = source.data[(base_pos.x + ((red_first_x + 1) % 2)) + (base_pos.y + ((red_first_y + 1) % 2)) * params.width * 2];
    sink.data[(pos.y * params.width + pos.x) * 3 + 1] = uint8_t(source.data[(base_pos.x + ((red_first_x + 1) % 2)) + (base_pos.y + ((red_first_y + 0) % 2)) * params.width * 2] / 2 + source.data[(base_pos.x + ((red_first_x + 0) % 2)) + (base_pos.y + ((red_first_y + 1) % 2)) * params.width * 2] / 2);
}
