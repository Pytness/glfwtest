#version 330 core
in vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_font;
uniform vec3 u_text_color;

void main() {
    vec3 coverage = texture(u_font, v_uv).rgb;
    vec3 color = u_text_color * coverage;
    float alpha = max(max(coverage.r, coverage.g), coverage.b);

    frag_color = vec4(color, alpha);
}
