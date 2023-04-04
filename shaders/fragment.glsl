#version 450

layout(location=0)in vec4 color_from_vertex_shader;

layout(location=0)out vec4 output_colour;

void main(){
    output_colour=color_from_vertex_shader;
}
