#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint  width;
    uint  height;
} params;

layout(set = 0, binding = 0) buffer readonly  Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint8_t data[]; } sink;
layout(set = 0, binding = 2) uniform sampler3D lut_sampler;

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy;
    if (pos.x >= params.width || pos.y >= params.height) return;
    uint idx = 3 * (params.width * pos.y + pos.x);
    uint8_t r = source.data[idx + 0];
    uint8_t g = source.data[idx + 1];
    uint8_t b = source.data[idx + 2];

    vec3 orig_rgb = vec3(r, g, b) / 255.0;
    vec4 rgba = texture(lut_sampler, orig_rgb.rgb) * 255.0;

    sink.data[idx + 0] = uint8_t(rgba.r);
    sink.data[idx + 1] = uint8_t(rgba.g);
    sink.data[idx + 2] = uint8_t(rgba.b);
}
