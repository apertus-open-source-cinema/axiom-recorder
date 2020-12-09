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

layout( set = 0, binding = 0, r8 ) uniform imageBuffer source;
layout( set = 0, binding = 1, rgba8 ) uniform imageBuffer sink;

float get_pixel_at(int x, int y) {
    return imageLoad(source, y * int(params.width) + x).r;
}

void write_rgb_at(int x, int y, float r, float g, float b) {
    imageStore(sink, (y * int(params.width)+ x) * 3 + 0, vec4(r, 0., 0., 0.));
    imageStore(sink, (y * int(params.width)+ x) * 3 + 1, vec4(g, 0., 0., 0.));
    imageStore(sink, (y * int(params.width)+ x) * 3 + 2, vec4(b, 0., 0., 0.));
}

float avrg4(float a, float b, float c, float d) {
    return (a / float(4) + b / float(4) + c / float(4) + d / float(4));
}

float avrg2(float a, float b) {
    return (a / float(2) + b / float(2));
}

void main() {
    int x = int(gl_GlobalInvocationID.x);
    int y = int(gl_GlobalInvocationID.y);

    bool x_red = (x + params.first_red_x) % 2 == 0;
    bool y_red = (y + params.first_red_y) % 2 == 0;

    if (x_red && y_red) {  // we are a red pixel
        write_rgb_at(x, y,
            get_pixel_at(x, y),
            avrg4(get_pixel_at(x+1, y), get_pixel_at(x-1, y), get_pixel_at(x, y+1), get_pixel_at(x, y-1)),
            avrg4(get_pixel_at(x+1, y+1), get_pixel_at(x-1, y-1), get_pixel_at(x-1, y+1), get_pixel_at(x+1, y-1))
        );
    } else if (!x_red && !y_red) {  // we are a blue pixel
        write_rgb_at(x, y,
            avrg4(get_pixel_at(x+1, y+1), get_pixel_at(x-1, y-1), get_pixel_at(x-1, y+1), get_pixel_at(x+1, y-1)),
            avrg4(get_pixel_at(x+1, y), get_pixel_at(x-1, y), get_pixel_at(x, y+1), get_pixel_at(x, y-1)),
            get_pixel_at(x, y)
        );
    } else if (!x_red && y_red) {  // we are a green pixel in a red row
        write_rgb_at(x, y,
            avrg2(get_pixel_at(x-1, y), get_pixel_at(x+1, y)),
            get_pixel_at(x, y),
            avrg2(get_pixel_at(x, y-1), get_pixel_at(x, y+1))
        );
    } else if (x_red && !y_red) {  // we are a green pixel in a blue row
        write_rgb_at(x, y,
            avrg2(get_pixel_at(x, y-1), get_pixel_at(x, y+1)),
            get_pixel_at(x, y),
            avrg2(get_pixel_at(x-1, y), get_pixel_at(x+1, y))
        );
    }
}
