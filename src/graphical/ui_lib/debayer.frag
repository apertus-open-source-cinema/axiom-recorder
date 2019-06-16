#version 450
uniform sampler2D raw_image;
out vec4 color;

float get_intensity(ivec2 pos) {
    float a = 0.021324;
    float in_bits = 12.0;
    float out_bits = 8.0;

    float x = texelFetch(raw_image, pos, 0).r * 256.0;

    float i = ((exp(x * log(a * (pow(2.0, in_bits)) - a + 1.0) / (pow(2.0, out_bits) - 1.0)) + a - 1.0) / a) - 1.0;

    return i / 4096.0;
}

vec3 get_color_value(ivec2 pos) {
    int x = (pos.x/2)*2;
    int y = (pos.y/2)*2;

    float r = get_intensity(ivec2(x + 1, y));
    float g1 = get_intensity(ivec2(x, y));
    float g2 = get_intensity(ivec2(x+1, y+1));
    float b = get_intensity(ivec2(x, y + 1));

    vec3 col = vec3(r, (g1+g2)/2.0, b);
    return col;
}


void main(void) {
    ivec2 size = textureSize(raw_image, 0);
    ivec2 icord = ivec2(gl_FragCoord) * ivec2(2);
    ivec2 rotcord = ivec2(size.x - icord.x, size.y - icord.y);

    vec3 debayered = get_color_value(rotcord);
    vec3 clamped = max(debayered, vec3(0.));
    vec3 powed = pow(clamped, vec3(0.5 * 2.));
    vec3 exposured = powed * 0.5 * 2.;

    // float i = get_intensity(ivec2(gl_FragCoord));
    // color = vec4(i, i, i, 1.0);

    // pack the color into the gl_FragColor without transparency
    color = vec4(exposured, 1.0);
    // color = vec4(1.0, 0.0, 0.0, 1.0);
}
