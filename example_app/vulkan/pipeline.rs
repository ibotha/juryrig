use ash::vk::{
    self, DescriptorSetLayoutBinding, DescriptorSetVariableDescriptorCountAllocateInfo,
    DescriptorSetVariableDescriptorCountAllocateInfoBuilder,
    DescriptorSetVariableDescriptorCountAllocateInfoEXT, PipelineDepthStencilStateCreateInfo,
    PushConstantRange,
};

use super::{error::RuntimeError, swapchain::Swapchain};
const MAX_IMAGES: u32 = 2;

pub(super) struct Pipeline {
    pub(super) pipeline: vk::Pipeline,
    pub(super) layout: vk::PipelineLayout,
    descriptor_pool: vk::DescriptorPool,
    pub(super) descriptor_sets: Vec<vk::DescriptorSet>,
    descriptor_set_layout_texture: vk::DescriptorSetLayout,
}

impl Pipeline {
    pub(super) fn cleanup(&self, logical_device: &ash::Device) {
        unsafe {
            logical_device.destroy_pipeline(self.pipeline, None);
            logical_device.destroy_pipeline_layout(self.layout, None);
            logical_device.destroy_descriptor_pool(self.descriptor_pool, None);
            logical_device.destroy_descriptor_set_layout(self.descriptor_set_layout_texture, None);
        }
    }

    pub(super) fn init(
        logical_device: &ash::Device,
        swapchain: &Swapchain,
        renderpass: &vk::RenderPass,
    ) -> Result<Pipeline, vk::Result> {
        let vertex_shader_create_info = vk::ShaderModuleCreateInfo::builder().code(
            vk_shader_macros::include_glsl!("./shaders/vertex.glsl", kind: vert),
        );
        let vertex_shader_module =
            unsafe { logical_device.create_shader_module(&vertex_shader_create_info, None)? };

        let fragment_shader_create_info = vk::ShaderModuleCreateInfo::builder().code(
            vk_shader_macros::include_glsl!("./shaders/fragment.glsl", kind: frag),
        );
        let fragment_shader_module =
            unsafe { logical_device.create_shader_module(&fragment_shader_create_info, None)? };

        let main_function_name = std::ffi::CString::new("main").unwrap();

        let vertex_shader_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_shader_module)
            .name(&main_function_name);
        let fragment_shader_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_shader_module)
            .name(&main_function_name);
        let shader_stages = vec![vertex_shader_stage.build(), fragment_shader_stage.build()];

        let vertex_attrib_descs = [
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .offset(0)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .offset(16)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(2)
                .offset(32)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(3)
                .offset(48)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(4)
                .offset(64)
                .format(vk::Format::R32_UINT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(1)
                .location(5)
                .offset(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(1)
                .location(6)
                .offset(12)
                .format(vk::Format::R32G32_SFLOAT)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(1)
                .location(7)
                .offset(20)
                .format(vk::Format::R32G32B32_SFLOAT)
                .build(),
        ];

        let vertex_binding_descs = [
            vk::VertexInputBindingDescription::builder()
                .binding(0)
                .stride(68)
                .input_rate(vk::VertexInputRate::INSTANCE)
                .build(),
            vk::VertexInputBindingDescription::builder()
                .binding(1)
                .stride(32)
                .input_rate(vk::VertexInputRate::VERTEX)
                .build(),
        ];

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_attrib_descs)
            .vertex_binding_descriptions(&vertex_binding_descs);
        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let tessellation_state = vk::PipelineTessellationStateCreateInfo::builder();

        let viewports = [vk::Viewport {
            x: 0.,
            y: 0.,
            width: swapchain.extent.width as f32,
            height: swapchain.extent.height as f32,
            min_depth: 0.,
            max_depth: 1.,
        }];
        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.extent,
        }];
        let viewport_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterizer_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .line_width(1.0)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .cull_mode(vk::CullModeFlags::NONE)
            .polygon_mode(vk::PolygonMode::FILL);

        let multisampler_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let colourblend_attachments = [vk::PipelineColorBlendAttachmentState::builder()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            .build()];
        let colourblend_info =
            vk::PipelineColorBlendStateCreateInfo::builder().attachments(&colourblend_attachments);

        let depth_stencil_state = PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

        let push_constant_ranges = [PushConstantRange::builder()
            .size(64)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .build()];

        let descriptor_binding_flags = [vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT];
        let mut descriptorset_layout_binding_flags =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::builder()
                .binding_flags(&descriptor_binding_flags);

        let layout_bindings = [DescriptorSetLayoutBinding::builder()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .binding(0)
            .descriptor_count(MAX_IMAGES)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build()];

        let descriptor_set_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&layout_bindings)
            .push_next(&mut descriptorset_layout_binding_flags);

        let descriptor_set_layout_texture = unsafe {
            logical_device.create_descriptor_set_layout(&descriptor_set_layout_info, None)
        }?;

        let descriptor_pool_sizes = [vk::DescriptorPoolSize::builder()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(MAX_IMAGES * 3 * 1024)
            .build()];

        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&descriptor_pool_sizes)
            .max_sets(3);

        let descriptor_pool =
            unsafe { logical_device.create_descriptor_pool(&descriptor_pool_info, None) }?;

        let desc_layouts_texture = vec![descriptor_set_layout_texture; 3];
        // TODO: Move this into the texture code to allocate as needed.
        let mut variable = DescriptorSetVariableDescriptorCountAllocateInfo::builder()
            .descriptor_counts(&[2, 2, 2]);
        let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&desc_layouts_texture)
            .push_next(&mut variable);

        let descriptor_sets =
            unsafe { logical_device.allocate_descriptor_sets(&descriptor_set_allocate_info) }?;

        let descriptor_set_layouts = [descriptor_set_layout_texture];

        let pipelinelayout_info = vk::PipelineLayoutCreateInfo::builder()
            .push_constant_ranges(&push_constant_ranges)
            .set_layouts(&descriptor_set_layouts);

        let pipelinelayout =
            unsafe { logical_device.create_pipeline_layout(&pipelinelayout_info, None) }?;

        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly_info)
            .tessellation_state(&tessellation_state)
            .viewport_state(&viewport_info)
            .rasterization_state(&rasterizer_info)
            .multisample_state(&multisampler_info)
            .color_blend_state(&colourblend_info)
            .depth_stencil_state(&depth_stencil_state)
            .layout(pipelinelayout)
            .render_pass(*renderpass)
            .subpass(0);

        let graphicspipeline = unsafe {
            logical_device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[pipeline_info.build()],
                    None,
                )
                .expect("A problem with the pipeline creation")
        }[0];
        unsafe {
            logical_device.destroy_shader_module(fragment_shader_module, None);
            logical_device.destroy_shader_module(vertex_shader_module, None);
        }
        Ok(Pipeline {
            pipeline: graphicspipeline,
            layout: pipelinelayout,
            descriptor_pool,
            descriptor_sets,
            descriptor_set_layout_texture,
        })
    }
}
