/*
 * Debayer images and half the resolution
*/

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
