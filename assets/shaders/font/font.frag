#version 330 core
in vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_font;
uniform vec3 u_background_color;
uniform vec3 u_text_color;
uniform bool u_is_color;

void main() {
    vec4 tex = texture(u_font, v_uv);

    if (u_is_color) {
        float alpha = tex.a;
        vec3 color = mix(u_background_color, tex.rgb, alpha);
        frag_color = vec4(color, alpha);
    } else {
        float alpha = max(max(tex.r, tex.g), tex.b);
        vec3 color = mix(u_background_color, u_text_color, alpha);
       frag_color = vec4(color, alpha);
    }
}
