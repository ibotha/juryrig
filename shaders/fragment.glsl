#version 450

layout(binding=0)uniform sampler2D tex_sampler;

layout(location=0)in vec2 uv_from_vertex_shader;

layout(location=0)out vec4 output_colour;

void main(){
    output_colour = texture(tex_sampler, uv_from_vertex_shader);
}
