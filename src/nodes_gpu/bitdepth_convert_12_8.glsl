#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 16, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint width;
} params;

layout(set = 0, binding = 0) buffer readonly  Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint8_t data[]; } sink;

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy;

    uint source_idx = pos.y * params.width * 3 / 2 + 3 * pos.x;
    uint8_t a = source.data[source_idx + 0];
    uint8_t b = source.data[source_idx + 1];
    uint8_t c = source.data[source_idx + 2];

    uint sink_idx = pos.y * params.width + 2 * pos.x;
    sink.data[sink_idx + 0] = a;
    sink.data[sink_idx + 1] = (b << 4) | (c >> 4);
}

/*
layout(set = 0, binding = 0) buffer readonly  Source { uint32_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint32_t data[]; } sink;

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy;

    uint source_idx = pos.y * params.width * 3 / 2 + 3 * pos.x;
    uint32_t a = source.data[source_idx + 0];
    uint32_t b = source.data[source_idx + 1];
    uint32_t c = source.data[source_idx + 2];

    uint sink_idx = pos.y * params.width + 2 * pos.x;
    sink.data[sink_idx + 0] = (a & 0xff000000) | ((a << 4) & 0x00ff0000) | ((a << 8) & 0x0000ff00) | ((b >> 20));
    sink.data[sink_idx + 1] = ((b << 16) & 0xff000000) | ((b << 20) & 0x00f00000) | ((c >> 12) & 0x000f0000) | ((c >> 8) & 0x0000ff00) | ((c >> 4) & 0x000000ff);
}
*/
