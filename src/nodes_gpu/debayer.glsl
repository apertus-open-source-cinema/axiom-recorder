dtype3 produce_pixel(uvec2 pos) {
    /*
    variables a-i are the neighbour pixels (we are e)
    a b c
    d e f
    g h i
    */

    dtype a = read_pixel(pos + uvec2(-1, -1));
    dtype b = read_pixel(pos + uvec2( 0, -1));
    dtype c = read_pixel(pos + uvec2(+1, -1));
    dtype d = read_pixel(pos + uvec2(-1,  0));
    dtype e = read_pixel(pos + uvec2( 0,  0));
    dtype f = read_pixel(pos + uvec2(+1,  0));
    dtype g = read_pixel(pos + uvec2(-1, +1));
    dtype h = read_pixel(pos + uvec2( 0, +1));
    dtype i = read_pixel(pos + uvec2(+1, +1));

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

    float x_red = float((pos.x + uint(!CFA_RED_IN_FIRST_COL) + 1) % 2);
    float x_red_not = float((pos.x + uint(!CFA_RED_IN_FIRST_COL)) % 2);
    float y_red = float((pos.y + uint(!CFA_RED_IN_FIRST_ROW) + 1) % 2);
    float y_red_not = float((pos.y + uint(!CFA_RED_IN_FIRST_ROW)) % 2);

    vec3 rgb = (
        + red_pixel * x_red * y_red
        + blue_pixel * x_red_not * y_red_not
        + green_pixel_red_row * x_red_not * y_red
        + green_pixel_blue_row * x_red * y_red_not
    );

    return rgb;
}

