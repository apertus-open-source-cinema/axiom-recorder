#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 16, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint  width;
    uint  height;
} params;

layout(set = 0, binding = 0) buffer readonly  Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint data[]; } sink;

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    if (pos.x * 2 >= params.width || pos.y >= params.height) return;

    uint raw_idx = pos.y * params.width * 3 / 2 + 3 * pos.x;
    uint a = uint(source.data[raw_idx + 0]);
    uint b = uint(source.data[raw_idx + 1]);
    uint c = uint(source.data[raw_idx + 2]);

    uint first = (a << 4) | (b >> 4);
    uint second = (b << 8) | c;
    atomicAdd(sink.data[first], 1);
    atomicAdd(sink.data[second], 1);
}
