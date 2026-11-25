#version 300 es
// Vortexine's OS 4.4: pantalla en escala de grises (Hyprland, decoration:screen_shader). Para acromatopsia
// o para quien prefiere ver sin color; lo que importa se distingue por brillo, iconos y texto.
precision highp float;
in vec2 v_texcoord;
uniform sampler2D tex;
out vec4 fragColor;

void main() {
    vec4 c = texture(tex, v_texcoord);
    float y = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    fragColor = vec4(vec3(y), c.a);
}
