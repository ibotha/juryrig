#version 450

layout(push_constant)uniform constants{
    mat4 proj;
}PushConstants;

layout(location=0)in mat4 model;
layout(location=4)in vec3 position;
layout(location=5)in vec2 uv;
layout(location=6)in vec3 normal;

layout(location=0)out vec2 uv_for_fragment_shader;

void main(){
    gl_Position=PushConstants.proj*model*vec4(position,1);
    uv_for_fragment_shader=uv;
    // color_for_fragment_shader=vec4(normal,1);
}
