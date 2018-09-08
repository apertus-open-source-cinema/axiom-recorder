#version 450
uniform sampler2D in_image;
out vec4 color;
in vec2 frag_position;


void main(void) {
    ivec2 pos = ivec2(textureSize(in_image, 0) * ((frag_position.xy + vec2(1)) * vec2(.5)));
    color = vec4(texelFetch(in_image, pos, 0));
}
