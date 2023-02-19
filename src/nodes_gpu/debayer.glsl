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

    dtype3 red_pixel = dtype3(
        e,
        (f + d + h + b) / 4.,
        (i + a + g + c) / 4.
    );
    dtype3 blue_pixel = dtype3(
        (i + a + g + c) / 4.,
        (f + d + h + b) / 4.,
        e
    );
    dtype3 green_pixel_red_row = dtype3(
        (d + f) / 2.,
        e,
        (b + h) / 2.
    );
    dtype3 green_pixel_blue_row = dtype3(
        (b + h) / 2.,
        e,
        (d + f) / 2.
    );

    dtype x_red = dtype((pos.x + uint(!CFA_RED_IN_FIRST_COL) + 1) % 2);
    dtype x_red_not = dtype((pos.x + uint(!CFA_RED_IN_FIRST_COL)) % 2);
    dtype y_red = dtype((pos.y + uint(!CFA_RED_IN_FIRST_ROW) + 1) % 2);
    dtype y_red_not = dtype((pos.y + uint(!CFA_RED_IN_FIRST_ROW)) % 2);

    dtype3 rgb = (
        + red_pixel * x_red * y_red
        + blue_pixel * x_red_not * y_red_not
        + green_pixel_red_row * x_red_not * y_red
        + green_pixel_blue_row * x_red * y_red_not
    );

    return rgb;
}

