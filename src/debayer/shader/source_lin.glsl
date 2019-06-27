/*
 * A source of data which is simply plain linear.
 * E.g. it is used in the AXIOM micro currently.
*/

float get_intensity(ivec2 pos) {
    return texelFetch(raw_image, pos, 0).r;
}
