use bevy::{
    asset::Asset,
    camera::visibility::ViewVisibility,
    ecs::system::{Commands, Local, Query},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    reflect::Reflect,
    render::{
        render_resource::{AsBindGroup, RenderPipelineDescriptor},
        sync_world::RenderEntity,
        Extract,
    },
    shader::ShaderRef,
    transform::components::GlobalTransform,
};

use crate::{
    pipeline::{
        BillboardPipelineKey, RenderBillboardImage, RenderBillboardMesh, DEF_LOCK_ROTATION,
        DEF_LOCK_Y,
    },
    text::RenderBillboard,
    utils::calculate_billboard_uniform,
    BillboardDepth, BillboardLockAxis, BillboardMesh, BillboardTexture,
};

#[derive(Asset, Reflect, AsBindGroup, Clone)]
#[bind_group_data(BillboardPipelineKey)]
pub struct BillboardMaterialExtension {
    key: BillboardPipelineKey,
}

impl From<&BillboardMaterialExtension> for BillboardPipelineKey {
    fn from(value: &BillboardMaterialExtension) -> Self {
        value.key
    }
}

impl Default for BillboardMaterialExtension {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl BillboardMaterialExtension {
    pub fn new(lock_axis: BillboardLockAxis) -> Self {
        let mut key = BillboardPipelineKey::empty();

        if lock_axis.y_axis {
            key |= BillboardPipelineKey::LOCK_Y;
        }
        if lock_axis.rotation {
            key |= BillboardPipelineKey::LOCK_ROTATION;
        }

        Self { key }
    }
}

#[cfg(feature = "webgl2")]
const _: () = {
    assert!(std::mem::size_of::<BillboardMaterial>() % std::mem::size_of::<u128>() == 0);
};

impl MaterialExtension for BillboardMaterialExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/billboard.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/billboard.wgsl".into()
    }

    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.contains(BillboardPipelineKey::LOCK_Y) {
            descriptor.vertex.shader_defs.push(DEF_LOCK_Y.into());
        }
        if key
            .bind_group_data
            .contains(BillboardPipelineKey::LOCK_ROTATION)
        {
            descriptor.vertex.shader_defs.push(DEF_LOCK_ROTATION.into());
        }

        Ok(())
    }
}

pub fn extract_billboard_texture(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    billboard_text_query: Extract<
        Query<(
            &RenderEntity,
            &ViewVisibility,
            &GlobalTransform,
            &BillboardMesh,
            &BillboardTexture,
            &BillboardDepth,
            Option<&BillboardLockAxis>,
        )>,
    >,
) {
    let mut batch = Vec::with_capacity(*previous_len);

    for (
        render_entity,
        visibility,
        global_transform,
        billboard_mesh,
        billboard_texture,
        &depth,
        lock_axis,
    ) in &billboard_text_query
    {
        if !visibility.get() {
            continue;
        }

        let uniform = calculate_billboard_uniform(global_transform, lock_axis);

        batch.push((
            render_entity.id(),
            (
                uniform,
                RenderBillboardMesh {
                    id: billboard_mesh.0.id(),
                },
                RenderBillboardImage {
                    id: billboard_texture.0.id(),
                },
                RenderBillboard {
                    depth,
                    lock_axis: lock_axis.copied(),
                },
            ),
        ));
    }

    *previous_len = batch.len();
    commands.insert_batch(batch);
}
