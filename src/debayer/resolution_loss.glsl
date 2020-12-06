#version 450

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;

layout(push_constant) uniform PushConstantData {
    uint width;
    uint height;

    // these are actual coordinates of the first red pixel (unlike everywhere else)
    uint first_red_x;
    uint first_red_y;
} params;

layout(set = 0, binding = 0) buffer Source { uint data[]; } source;
layout(set = 0, binding = 1) buffer Sink   { uint data[]; } sink;

uint get_pixel_at(uint x, uint y) {
    return source.data[y * params.width + x];
}

void write_rgb_at(uint x, uint y, uint r, uint g, uint b) {
    sink.data[y * params.width * 3 + x * 3 + 0] = r;
    sink.data[y * params.width * 3 + x * 3 + 1] = g;
    sink.data[y * params.width * 3 + x * 3 + 2] = b;
}

uint avrg4(uint a, uint b, uint c, uint d) {
    return (a + b + c + d) / 4;
}

uint avrg2(uint a, uint b) {
    return (a + b) / 2;
}

void main() {
    uint x = gl_GlobalInvocationID.x;
    uint y = gl_GlobalInvocationID.y;

    if (x > params.width || y > params.height) {
        return;
    }

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