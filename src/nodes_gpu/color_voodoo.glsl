#version 450
#extension GL_EXT_shader_explicit_arithmetic_types: enable
#extension GL_EXT_shader_explicit_arithmetic_types_int8: require

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    float pedestal;
    float s_gamma;
    float v_gamma;
    uint  width;
    uint  height;
} params;

layout(set = 0, binding = 0) buffer readonly  Source { uint8_t data[]; } source;
layout(set = 0, binding = 1) buffer writeonly Sink   { uint8_t data[]; } sink;

// stolen from: https://stackoverflow.com/questions/15095909/from-rgb-to-hsv-in-opengl-glsl
vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));

    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void main() {
    uvec2 pos = gl_GlobalInvocationID.xy;
    if (pos.x >= params.width || pos.y >= params.height) return;
    uint idx = 3 * (params.width * pos.y + pos.x);
    uint8_t r = source.data[idx + 0];
    uint8_t g = source.data[idx + 1];
    uint8_t b = source.data[idx + 2];
    vec3 rgb = (vec3(r, g, b) - params.pedestal) / (256 - params.pedestal);
    vec3 hsv = rgb2hsv(rgb);
    hsv.g = pow(hsv.g, params.s_gamma);
    hsv.b = pow(hsv.b, params.v_gamma);
    rgb = hsv2rgb(hsv) * 256;

    sink.data[idx + 0] = uint8_t(rgb.r);
    sink.data[idx + 1] = uint8_t(rgb.g);
    sink.data[idx + 2] = uint8_t(rgb.b);
}
