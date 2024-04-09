#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require
#extension GL_EXT_debug_printf : enable

precision highp float;

layout(local_size_x = 16, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint  width;
    uint  height;
} params;

layout(set = 0, binding = 0) buffer readonly  Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint8_t data[]; } sink;
layout(set = 0, binding = 2) uniform sampler2D darkframe_sampler;

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    if (pos.x * 2 >= params.width || pos.y >= params.height) return;

    uint raw_idx = pos.y * params.width * 3 / 2 + 3 * pos.x;
    uint a = uint(source.data[raw_idx + 0]);
    uint b = uint(source.data[raw_idx + 1]);
    uint c = uint(source.data[raw_idx + 2]);

    float first_value = float((a << 4) | (b >> 4)) + 128.0;
    float second_value = float(((b << 8) & 0xf00) | c) + 128.0;

    float corr_first_v = texelFetch(darkframe_sampler, pos * ivec2(2, 1), 0).r;
    float corr_second_v = texelFetch(darkframe_sampler, pos * ivec2(2, 1) + ivec2(1, 0), 0).r;
    uint corr_first = uint(round(first_value - corr_first_v));
    uint corr_second = uint(round(second_value - corr_second_v));

    uint8_t a_corr = uint8_t(corr_first >> 4);
    uint8_t b_corr = uint8_t(((corr_first << 4) & 0xf0) | (corr_second >> 8));
    uint8_t c_corr = uint8_t(corr_second);

    sink.data[raw_idx + 0] = a_corr;
    sink.data[raw_idx + 1] = b_corr;
    sink.data[raw_idx + 2] = c_corr;
}
