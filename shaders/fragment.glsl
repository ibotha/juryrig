#version 450
#extension GL_EXT_nonuniform_qualifier : require

layout(set=0,binding=0)uniform sampler2D tex_samplers[];

layout(location=0)in vec2 uv_from_vertex_shader;
layout(location=1)in flat uint tex_id_from_vertex_shader;

layout(location=0)out vec4 output_colour;

void main(){
    output_colour = texture(tex_samplers[tex_id_from_vertex_shader], uv_from_vertex_shader) * vec4(tex_id_from_vertex_shader, 1,1,1);
}
