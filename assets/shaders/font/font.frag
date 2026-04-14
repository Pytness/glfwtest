#version 330 core
in vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_font;
uniform vec3 u_background_color;
uniform vec3 u_text_color;
uniform bool u_is_color;

void main() {
    if (u_is_color) {
        vec4 coverage = texture(u_font, v_uv).rgba;
        float alpha = max(max(coverage.r, coverage.g), coverage.b);

        // Output the text color with the alpha from the glyph texture.
        frag_color = coverage;
    } else {
        vec3 coverage = texture(u_font, v_uv).rgb;
        float alpha = max(max(coverage.r, coverage.g), coverage.b);

        // Output the text color with the alpha from the glyph texture.
        frag_color = vec4(u_text_color, alpha);
    }

}
