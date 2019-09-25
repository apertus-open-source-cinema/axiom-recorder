/*
 * A source of data which is simply plain linear.
 * E.g. it is used in the AXIOM micro currently.
*/

ivec2 size_source = textureSize(texture, 0);

float get_intensity(ivec2 pos) {
    return texelFetch(texture, ivec2(pos.x, size_source.y - pos.y), 0).r;
}
