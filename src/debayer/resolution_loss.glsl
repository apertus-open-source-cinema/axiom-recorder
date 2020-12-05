#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer Source { uint data[]; } source;
layout(set = 0, binding = 1) buffer Sink   { uint data[]; } sink;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    sink.data[idx] = source.data[idx];
}