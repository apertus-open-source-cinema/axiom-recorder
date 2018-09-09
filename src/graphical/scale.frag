#version 450
uniform sampler2D in_image;
out vec4 color;
in vec2 surface_position;

void main(void) {
    ivec2 ts = textureSize(in_image, 0);

    ivec2 pos = ivec2(greater_dim * ((surface_position.xy + vec2(1)) * vec2(.5)));
    color = vec4(texelFetch(in_image, pos, 0));
}
