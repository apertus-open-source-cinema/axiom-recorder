#version 450

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint width;
    uint height;
} params;

layout(set = 0, binding = 0) buffer Source { uint data[]; } source;
layout(set = 0, binding = 1) buffer Sink   { uint data[]; } sink;

uint get_pixel_at(uint x, uint y) {
    return source.data[y * params.width + x];
}

void main() {
    uint x = gl_GlobalInvocationID.x;
    uint y = gl_GlobalInvocationID.y;

    if (x > params.width || y > params.height) {
        return;
    }

    sink.data[y * params.width + x] = get_pixel_at(x, y);
}