#version 450

layout(push_constant)uniform constants{
    mat4 proj;
}PushConstants;

layout(location=0)in vec4 position;
layout(location=1)in vec4 color;

layout(location=0)out vec4 color_for_fragment_shader;

void main(){
    gl_Position=PushConstants.proj*position;
    color_for_fragment_shader=color;
}
