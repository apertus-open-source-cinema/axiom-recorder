#version 450
uniform sampler2D raw_image;
/*
* The raw_image is packed in a wired way:
* 4 Pixels of one Line are bound together
*/
out vec4 color;

vec3 get_color_value(ivec2 pos) {
    int x = pos.x / 2;
    int x_base = (x/2)*2;
    int y = pos.y;

    vec4 line1 = texelFetch(raw_image, ivec2(x_base, y), 0);
    vec4 line2 = texelFetch(raw_image, ivec2(x_base, y + 1), 0);

    float r, g1, g2, b;
    if(x == x_base) {
        r = line1.r;
        g1 = line1.g;
        g2 = line2.r;
        b = line2.g;
    } else {
        r = line1.b;
        g1 = line1.a;
        g2 = line2.b;
        b = line2.a;
    }

    vec3 col = vec3(r, (g1+g2)/2.0, b);
    return col;
}


void main(void) {
    ivec2 size = textureSize(raw_image, 0);
    ivec2 icord = ivec2(gl_FragCoord);
    ivec2 rotcord = ivec2(size.x - icord.x, icord.y);

    vec3 debayered = get_color_value(rotcord);

    color = vec4(debayered, 1.0);
}