#version 330 core
in vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_font;
uniform vec3 u_text_color;

void main() {
    // The glyph texture is GL_RED (single channel); sample only the red component.
    float coverage = texture(u_font, v_uv).r;

    frag_color = vec4(u_text_color * coverage, coverage);
}
