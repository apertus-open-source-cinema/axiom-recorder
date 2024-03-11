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

    /*
    variables a-i are the neighbour pixels (we are e)
    a b c
    d e f
    g h i
    */

    float a = float(source.data[(pos.x - 1) + (pos.y - 1) * params.width]);
    float b = float(source.data[(pos.x    ) + (pos.y - 1) * params.width]);
    float c = float(source.data[(pos.x + 1) + (pos.y - 1) * params.width]);
    float d = float(source.data[(pos.x - 1) + (pos.y    ) * params.width]);
    float e = float(source.data[(pos.x    ) + (pos.y    ) * params.width]);
    float f = float(source.data[(pos.x + 1) + (pos.y    ) * params.width]);
    float g = float(source.data[(pos.x - 1) + (pos.y + 1) * params.width]);
    float h = float(source.data[(pos.x    ) + (pos.y + 1) * params.width]);
    float i = float(source.data[(pos.x + 1) + (pos.y + 1) * params.width]);

    vec3 red_pixel = vec3(
        e,
        (f + d + h + b) / 4.,
        (i + a + g + c) / 4.
    );
    vec3 blue_pixel = vec3(
        (i + a + g + c) / 4.,
        (f + d + h + b) / 4.,
        e
    );
    vec3 green_pixel_red_row = vec3(
        (d + f) / 2.,
        e,
        (b + h) / 2.
    );
    vec3 green_pixel_blue_row = vec3(
        (b + h) / 2.,
        e,
        (d + f) / 2.
    );

    float x_red = float((pos.x + params.first_red_x + 1) % 2);
    float x_red_not = float((pos.x + params.first_red_x) % 2);
    float y_red = float((pos.y + params.first_red_y + 1) % 2);
    float y_red_not = float((pos.y + params.first_red_y) % 2);

    vec3 rgb = (
        + red_pixel * x_red * y_red
        + blue_pixel * x_red_not * y_red_not
        + green_pixel_red_row * x_red_not * y_red
        + green_pixel_blue_row * x_red * y_red_not
    );

    sink.data[(pos.y * params.width + pos.x) * 3 + 0] = uint8_t(rgb.r);
    sink.data[(pos.y * params.width + pos.x) * 3 + 1] = uint8_t(rgb.g);
    sink.data[(pos.y * params.width + pos.x) * 3 + 2] = uint8_t(rgb.b);
}

