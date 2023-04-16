#version 450
#extension GL_EXT_nonuniform_qualifier : require

layout(set=0,binding=0)uniform sampler2D tex_samplers[];

layout(location=0)in vec2 uv_from_vertex_shader;
layout(location=1)in vec3 normal_from_vertex_shader;
layout(location=2)in flat uint tex_id_from_vertex_shader;


layout(location=0)out vec4 output_colour;

void main(){
    vec4 albedo = texture(tex_samplers[tex_id_from_vertex_shader], uv_from_vertex_shader);
    // This is the same as above but with a hard-coded branch to select the texture: This works properly
    // vec4 albedo;
    // if (tex_id_from_vertex_shader == 0) {
    //     albedo = texture(tex_samplers[0], uv_from_vertex_shader);
    // } else {
    //     albedo = texture(tex_samplers[1], uv_from_vertex_shader);
    // }
    float light = clamp(dot(normal_from_vertex_shader, normalize(vec3(1,1,1))), 0.2, 1);
    output_colour =  vec4(albedo.rgb * light, albedo.a);
}
