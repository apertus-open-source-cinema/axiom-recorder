ivec2 size_debayer = textureSize(texture, 0);

vec3 get_color_value(ivec2 pos) {
    // define the intensitys of the beyer pattern
	/*
    vec3 r = vec3(1., 0., 0.) * 0.25;
    vec3 g = vec3(0., 1., 0.) * 0.125;
    vec3 b = vec3(0., 0., 1.) * 0.25;
	*/


	vec3 a = vec3(get_intensity(pos + ivec2(-1, -1)), get_intensity(pos + ivec2(0, -1)), get_intensity(pos + ivec2(1, -1)));
	vec3 b = vec3(get_intensity(pos + ivec2(-1,  0)), get_intensity(pos + ivec2(0,  0)), get_intensity(pos + ivec2(1,  0)));
	vec3 c = vec3(get_intensity(pos + ivec2(-1,  1)), get_intensity(pos + ivec2(0,  1)), get_intensity(pos + ivec2(1,  1)));

    vec3 pixel_color = vec3(0.0);

	int off = (pos.x % 2) + 2 * (pos.y % 2);


/*
	RGRGRGRG
	GBGBGBGB
	RGRGRGRG
	GBGBGBGB
	*/

	if (off == 0) {
//	   pixel_color = vec3(b.y, 0.0, 0.0);
	   pixel_color = vec3(b.y, 0.25 * (a.y + b.x + b.z + c.y), 0.25 * (a.x + a.z + c.x + c.z));
	} else if (off == 1) {
//	   pixel_color = vec3(0.0, b.y, 0.0);
	   pixel_color = vec3(0.5 * (b.x + b.z), 0.2 * (a.x + a.z + c.x + c.z + b.y), 0.5 * (a.y + c.y));
	} else if (off == 2) {
//	   pixel_color = vec3(0.0, b.y, 0.0);
	   pixel_color = vec3(0.5 * (a.y + c.y), 0.2 * (a.x + a.z + c.x + c.z + b.y), 0.5 * (b.x + b.z));
	} else {
//	   pixel_color = vec3(0.0, 0.0, b.y);
	   pixel_color = vec3(0.25 * (a.x + a.z + c.x + c.z), 0.25 * (a.y + b.x + b.z + c.y), b.y);
	}

	return pixel_color;
}