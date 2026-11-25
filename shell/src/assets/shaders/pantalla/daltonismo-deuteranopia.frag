#version 300 es
// Vortexine's OS 4.4: filtro de pantalla para deuteranopia (verde) (Hyprland, decoration:screen_shader).
// Daltonización (Fidaner, Lin y Ozguven): se simula lo que no se alcanza a ver, y la diferencia se pasa
// a los colores que sí se distinguen. Corrige todo lo que sale en pantalla: escritorio, apps y juegos.
precision highp float;
in vec2 v_texcoord;
uniform sampler2D tex;
out vec4 fragColor;

void main() {
    vec4 c = texture(tex, v_texcoord);
    vec3 rgb = c.rgb;
    // RGB → LMS
    float L = 17.8824 * rgb.r + 43.5161 * rgb.g + 4.11935 * rgb.b;
    float M = 3.45565 * rgb.r + 27.1554 * rgb.g + 3.86714 * rgb.b;
    float S = 0.0299566 * rgb.r + 0.184309 * rgb.g + 1.46709 * rgb.b;
    // Lo que se ve con deuteranopia (verde)
    float l = L;
    float m = 0.494207 * L + 1.24827 * S;
    float s = S;
    // LMS → RGB
    vec3 vista = vec3(
        0.0809444479 * l - 0.130504409 * m + 0.116721066 * s,
        -0.0102485335 * l + 0.0540193266 * m - 0.113614708 * s,
        -0.000365296938 * l - 0.00412161469 * m + 0.693511405 * s);
    // Lo que se pierde se pasa a los canales que sí se ven.
    vec3 error = rgb - vista;
    vec3 ajuste = vec3(0.0, error.r * 0.7 + error.g, error.r * 0.7 + error.b);
    fragColor = vec4(clamp(rgb + ajuste, 0.0, 1.0), c.a);
}
