#version 330 core

in vec2 p;
out vec2 uv;

void main() {
    gl_Position = vec4(p, 0.0, 1.0);
    uv = p;
}
