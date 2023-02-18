layout(push_constant) uniform PushConstantData {
    dtype pedestal;
    dtype s_gamma;
    dtype v_gamma;
} params;


// stolen from: https://stackoverflow.com/questions/15095909/from-rgb-to-hsv-in-opengl-glsl
dtype3 rgb2hsv(dtype3 c) {
    dtype4 K = dtype4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    dtype4 p = mix(dtype4(c.bg, K.wz), dtype4(c.gb, K.xy), step(c.b, c.g));
    dtype4 q = mix(dtype4(p.xyw, c.r), dtype4(c.r, p.yzx), step(p.x, c.r));

    dtype d = q.x - min(q.w, q.y);
    dtype e = 1.0e-10;
    return dtype3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}
dtype3 hsv2rgb(dtype3 c) {
    dtype4 K = dtype4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    dtype3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

dtype3 produce_pixel(uvec2 pos) {
    dtype3 rgb = read_pixel(pos);
    rgb = (rgb - params.pedestal) / (1 - params.pedestal);
    dtype3 hsv = rgb2hsv(rgb);
    hsv.g = pow(hsv.g, params.s_gamma);
    hsv.b = pow(hsv.b, params.v_gamma);
    rgb = hsv2rgb(hsv);
    return rgb;
}
