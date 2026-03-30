#version 330 core
in vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_font;
uniform vec3 u_text_color;

void main() {
    float alpha = texture(u_font, v_uv).r;
    frag_color = vec4(u_text_color, alpha);
}
