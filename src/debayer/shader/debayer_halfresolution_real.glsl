/*
 * Debayer images and half the resolution
 *
 * ! resolution_div=2
*/

ivec2 size_debayer = textureSize(texture, 0);

vec3 get_color_value(ivec2 pos) {
    int x =	int(pos.x * 2);
    int y =	int(pos.y * 2 - size_debayer.y);

/*
	float r  = get_intensity(ivec2(2 * x + 0, 2 * y + 0));
	float g1 = get_intensity(ivec2(2 * x + 1, 2 * y + 0));
	float g2 = get_intensity(ivec2(2 * x + 0, 2 * y + 1));
	float b  = get_intensity(ivec2(2 * x + 1, 2 * y + 1));
	*/

    float r = get_intensity(ivec2(x, y));
    float g1 = get_intensity(ivec2(x + 1, y));
    float g2 = get_intensity(ivec2(x, y+1));
    float b = get_intensity(ivec2(x + 1, y + 1));

    return vec3(r, (g1+g2)/2.0, b);
    // return vec3(1.0, 0.0, 0.0);
//    return vec3((float) pos.x / 256.0, (float) pos.y / 256.0, 0.0);
}
