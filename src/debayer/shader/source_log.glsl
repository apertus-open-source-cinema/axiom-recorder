/*
 * A source of data which was compressed with some log algorithm.
 * E.g. it is used in the AXIOM micro currently.
*/

uniform float a; // = 0.021324
uniform float in_bits; // = 12.0
uniform float out_bits; // = 8.0

float get_intensity(ivec2 pos) {
    float x = texelFetch(raw_image, pos, 0).r * 256.0;

    float i = ((exp(x * log(a * (pow(2.0, in_bits)) - a + 1.0) / (pow(2.0, out_bits) - 1.0)) + a - 1.0) / a) - 1.0;

    return i / 4096.0;
}