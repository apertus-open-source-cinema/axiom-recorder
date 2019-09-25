vec3 get_color_value(ivec2 pos) {
    // define the intensitys of the beyer pattern
    vec3 r = vec3(1., 0., 0.);
    vec3 g = vec3(0., 1., 0.);
    vec3 b = vec3(0., 0., 1.);

    vec3 pixel_color = vec3(0.0);

	ivec2 offset = ivec2(pos.x % 4, 0);

	// rggb bayer pattern
	pixel_color += r * get_intensity(pos - offset + ivec2(0, 0));
	pixel_color += (g * get_intensity(pos - offset + ivec2(1, 0)) + g * get_intensity(pos - offset + ivec2(2, 0))) * 0.5;
	pixel_color += b * get_intensity(pos - offset + ivec2(3, 0));

    if(pos.x % 2 == 1 && pos.y % 2 == 0) {
        // red sensel
        pixel_color += r * get_intensity(pos - ivec2(0, 0));
        pixel_color += g * (get_intensity(pos - ivec2(0, 1)) + get_intensity(pos - ivec2(1, 0)) + get_intensity(pos - ivec2(-1, 0)) + get_intensity(pos - ivec2(0, -1))) / 4.;
        pixel_color += b * (get_intensity(pos - ivec2(1, 1)) + get_intensity(pos - ivec2(-1, -1)) + get_intensity(pos - ivec2(-1, 1)) + get_intensity(pos - ivec2(-1, 1))) / 4.;
    } else if (pos.x % 2 == 0 && pos.y % 2 == 1) {
        // blue sensel
        pixel_color += r * (get_intensity(pos - ivec2(1, 1)) + get_intensity(pos - ivec2(-1, -1)) + get_intensity(pos - ivec2(-1, 1)) + get_intensity(pos - ivec2(-1, 1))) / 4.;
        pixel_color += g * (get_intensity(pos - ivec2(0, 1)) + get_intensity(pos - ivec2(1, 0)) + get_intensity(pos - ivec2(-1, 0)) + get_intensity(pos - ivec2(0, -1))) / 4.;
        pixel_color += b * get_intensity(pos - ivec2(0, 0));
    } else if (pos.x % 2 == pos.y % 2) {
        // green sensel
        pixel_color += g * get_intensity(pos - ivec2(0, 0));
        if(pos.y % 2 == 0) {
            pixel_color += b * (get_intensity(pos - ivec2(0, 1)) + get_intensity(pos - ivec2(0, -1))) / 2.;
            pixel_color += r * (get_intensity(pos - ivec2(1, 0)) + get_intensity(pos - ivec2(-1, 0))) / 2.;
        } else {
            pixel_color += r * (get_intensity(pos - ivec2(0, 1)) + get_intensity(pos - ivec2(0, -1))) / 2.;
            pixel_color += b * (get_intensity(pos - ivec2(1, 0)) + get_intensity(pos - ivec2(-1, 0))) / 2.;
        }
    }

    return pixel_color;
}
