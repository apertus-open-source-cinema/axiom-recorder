const mat3[8] combiners = mat3[8](
mat3(0.0,  0.0,  0.25,
     0.0,  0.25, 0.0,
	 0.0, 0.0,  0.25),
mat3(0.0,  0.25, 0.0,
     1.0, 0.0, 0.0,
	 0.0,  0.25, 0.0),
mat3(0.0,  0.2,  0.0,
     0.0,  0.0, 0.5,
	 0.0,  0.2,  0.0),
mat3(0.5,  0.0, 0.0,
     0.0,  0.2, 0.0,
	 0.5,  0.0, 0.0),
mat3(0.0,  0.2,  0.0,
     0.5,  0.0, 0.0,
	 0.0,  0.2,  0.0),
mat3(0.0,  0.0, 0.5,
     0.0,  0.2, 0.0,
	 0.0,  0.0, 0.5),
mat3(0.25,  0.0,  0.0,
     0.0,  0.25, 0.0,
	 0.25, 0.0,  0.0),
mat3(0.0,  0.25, 0.0,
     0.0, 0.0, 1.0,
	 0.0,  0.25, 0.0));
/*
const mat3[8] combiners = mat3[8](
mat3(0.0,  0.0,  0.0,
     0.0,  0.25, 0.0,
	 0.25, 0.0,  0.25),
mat3(0.0,  1.0, 0.0,
     0.25, 0.0, 0.25,
	 0.0,  0.0, 0.0),
mat3(0.0,  0.0,  0.0,
     0.2,  0.0, 0.2,
	 0.0,  0.5,  0.5),
mat3(0.5,  0.0, 0.5,
     0.0,  0.2, 0.0,
	 0.0,  0.0, 0.0),
mat3(0.0,  0.5,  0.0,
     0.2,  0.0, 0.2,
	 0.0,  0.0,  0.5),
mat3(0.0,  0.0, 0.0,
     0.0,  0.2, 0.0,
	 0.5,  0.0, 0.5),
mat3(0.25,  0.0,  0.25,
     0.0,  0.25, 0.0,
	 0.0, 0.0,  0.0),
mat3(0.0,  0.0, 0.0,
     0.25, 0.0, 0.25,
	 0.0,  1.0, 0.0));
	 */

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

	int off = 2 * (pos.x % 2) + 4 * (pos.y % 2);

	pixel_color += combiners[off + 0] * a;
	pixel_color += combiners[off + 1] * b;
	pixel_color += combiners[off + 0] * c;

	return pixel_color;
}



/*

	RGRGRGRG
	GBGBGBGB
	RGRGRGRG
	GBGBGBGB
*/


/*

	// ivec2 pos = size_debayer.x
	pos = ivec2(((pos.x % (size_debayer.x / 2)) / 2) * 4, pos.x / (size_debayer.x / 2) + (pos.y / 2) * 2);

	// ivec2 offset = ivec2(pos.x % 4, 0);


	ivec2 pos_a = (pos / 4) * 4;

	ivec2 pos_b = pos_a + ivec2(4, 0);
	ivec2 pos_c = pos_a + ivec2(0, 1);
	ivec2 pos_d = pos_a + ivec2(4, 1);


	RGGBRGGBRGGBRGGB
	RGGBRGGBRGGBRGGB
	RGGBRGGBRGGBRGGB
	RGGBRGGBRGGBRGGB




	// rggb bayer pattern
	pixel_color += r * (get_intensity(pos_a + ivec2(0, 0)) + get_intensity(pos_b + ivec2(0, 0)) + get_intensity(pos_c + ivec2(0, 0)) + get_intensity(pos_d + ivec2(0, 0)));
	pixel_color += g * (get_intensity(pos_a + ivec2(1, 0)) + get_intensity(pos_a + ivec2(2, 0))); // + get_intensity(pos_b + ivec2(1, 0)) + get_intensity(pos_b + ivec2(2, 0)) + get_intensity(pos_c + ivec2(1, 0)) + get_intensity(pos_c + ivec2(2, 0)) + get_intensity(pos_d + ivec2(1, 0)) + get_intensity(pos_d + ivec2(2, 0)));
	pixel_color += b * (get_intensity(pos_a + ivec2(3, 0)) + get_intensity(pos_b + ivec2(3, 0)) + get_intensity(pos_c + ivec2(3, 0)) + get_intensity(pos_d + ivec2(3, 0)));

    return pixel_color;
	*/
