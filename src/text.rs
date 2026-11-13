use crate::pipeline::{RenderBillboardImage, RenderBillboardMesh};
use crate::utils::calculate_billboard_uniform;
use crate::{BillboardDepth, BillboardLockAxis, BillboardText, BillboardTextNeedsRerender};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashSet;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::sync_world::RenderEntity;
use bevy::render::Extract;
use bevy::sprite::Anchor;
use bevy::text::{
    ComputedTextBlock, FontAtlasSet, FontCx, FontHinting, LayoutCx, PositionedGlyph, RemSize,
    ScaleCx, TextBounds, TextElement, TextError, TextLayoutInfo, TextPipeline, TextReader,
};
use bevy::window::PrimaryWindow;
use smallvec::SmallVec;

// Uses this as reference
// https://github.com/bevyengine/bevy/blob/v0.11.2/crates/bevy_text/src/text2d.rs

#[derive(Component, Copy, Clone, Debug, Reflect, Deref, Default)]
#[reflect(Component)]
pub struct BillboardTextBounds(pub TextBounds);

// TODO: Maybe use something like { Single(Group), Multi(SmallVec<[Group; 1]>) }, benchmark it
#[derive(Component, Clone, Debug, Deref, DerefMut, Default, Reflect)]
#[reflect(Component)]
pub struct BillboardTextHandles(pub SmallVec<[BillboardTextHandleGroup; 1]>);

#[derive(Clone, Debug, Default, Reflect)]
pub struct BillboardTextHandleGroup {
    mesh: Handle<Mesh>,
    image: AssetId<Image>,
}

pub fn extract_billboard_text(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    billboard_text_query: Extract<
        Query<(
            &RenderEntity,
            &ViewVisibility,
            &GlobalTransform,
            &BillboardTextHandles,
            &BillboardDepth,
            Option<&BillboardLockAxis>,
        )>,
    >,
) {
    let mut batch = Vec::with_capacity(*previous_len);

    for (render_entity, visibility, global_transform, handles, &depth, lock_axis) in
        &billboard_text_query
    {
        if !visibility.get() {
            continue;
        }

        let uniform = calculate_billboard_uniform(global_transform, lock_axis);

        for handle_group in handles.iter() {
            // TODO: this will overwrite the render entity if we try to
            // TODO: add multiple handles in the same extraction!
            batch.push((
                render_entity.id(),
                (
                    uniform,
                    RenderBillboardMesh {
                        id: handle_group.mesh.id(),
                    },
                    RenderBillboardImage {
                        id: handle_group.image,
                    },
                    RenderBillboard {
                        depth,
                        lock_axis: lock_axis.copied(),
                    },
                ),
            ));
        }
    }

    *previous_len = batch.len();
    commands.insert_batch(batch);
}

pub(crate) fn update_billboard_text_layout(
    mut queue: Local<EntityHashSet>,
    mut last_logical_viewport_size: Local<Vec2>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    fonts: Res<Assets<Font>>,
    mut font_atlas_set: ResMut<FontAtlasSet>,
    mut text_pipeline: ResMut<TextPipeline>,
    mut font_cx: ResMut<FontCx>,
    mut layout_cx: ResMut<LayoutCx>,
    mut scale_cx: ResMut<ScaleCx>,
    rem_size: Res<RemSize>,
    primary_window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut text_query: Query<
        (
            Entity,
            Ref<BillboardText>,
            &mut TextLayoutInfo,
            Ref<TextLayout>,
            Ref<BillboardTextBounds>,
            Ref<Anchor>,
            &mut BillboardTextHandles,
            &mut ComputedTextBlock,
            Has<BillboardTextNeedsRerender>,
        ),
        With<BillboardText>,
    >,
    mut text_reader: TextReader<BillboardText>,
    mut commands: Commands,
) {
    const SCALE_FACTOR: f32 = 1.0;

    let logical_viewport_size = primary_window
        .map(|window| window.resolution.size())
        .unwrap_or(Vec2::splat(1000.));
    let viewport_size_changed = *last_logical_viewport_size != logical_viewport_size;
    *last_logical_viewport_size = logical_viewport_size;

    for (
        entity,
        billboard_text,
        mut info,
        layout,
        bounds,
        anchor,
        mut handles,
        mut computed,
        text_needs_rerender,
    ) in &mut text_query
    {
        // `bevy::text::detect_text_needs_rerender` does not check root text components, so
        // changes to the `BillboardText` string itself have to be detected here.
        if billboard_text.is_changed()
            || text_needs_rerender
            || layout.is_changed()
            || bounds.is_changed()
            || anchor.is_changed()
            || computed.needs_rerender(viewport_size_changed, rem_size.is_changed())
            || (!queue.is_empty() && queue.remove(&entity))
        {
            let text_bounds = if layout.linebreak == LineBreak::NoWrap {
                TextBounds::UNBOUNDED
            } else {
                bounds.0
            };

            match text_pipeline.update_buffer(
                &fonts,
                text_reader.iter(entity),
                layout.linebreak,
                layout.justify,
                text_bounds,
                SCALE_FACTOR,
                &mut computed,
                &mut font_cx,
                &mut layout_cx,
                logical_viewport_size,
                *rem_size,
            ) {
                Err(
                    TextError::NoSuchFont
                    | TextError::NoSuchFontFamily(_)
                    | TextError::DegenerateScaleFactor,
                ) => {
                    // Font could still be loading, retry next frame.
                    queue.insert(entity);
                    continue;
                }
                Err(err) => {
                    panic!("Fatal error when processing text: {err}.");
                }
                Ok(()) => (),
            };

            match text_pipeline.update_text_layout_info(
                &mut info,
                &mut font_atlas_set,
                &mut images,
                &mut computed,
                &mut scale_cx,
                text_bounds,
                layout.justify,
                FontHinting::default(),
            ) {
                Err(TextError::NoSuchFont | TextError::NoSuchFontFamily(_)) => {
                    queue.insert(entity);
                    continue;
                }
                Err(err) => {
                    panic!("Fatal error when processing text: {err}.");
                }
                Ok(()) => (),
            };

            commands
                .entity(entity)
                .remove::<BillboardTextNeedsRerender>();

            // Colors indexed by `PositionedGlyph::section_index`, which follows the
            // iteration order of `TextReader::iter`.
            let section_colors: SmallVec<[[f32; 4]; 4]> = text_reader
                .iter(entity)
                .map(|(_, _, element)| match element {
                    TextElement::Text { color, .. } => color.to_linear().to_f32_array(),
                    TextElement::Box(_) => LinearRgba::WHITE.to_f32_array(),
                })
                .collect();

            let text_anchor = -(anchor.as_vec() + 0.5);
            let alignment_translation = info.size * text_anchor;

            let length = info.glyphs.len();
            let mut textures: HashMap<AssetId<Image>, Vec<&PositionedGlyph>> = HashMap::new();

            for glyph in &info.glyphs {
                textures
                    .entry(glyph.atlas_info.texture)
                    .or_insert_with(|| Vec::with_capacity(length))
                    .push(glyph);
            }

            handles.clear();

            for (texture, glyphs) in textures {
                let Some(atlas_size) = images.get(texture).map(|image| image.size_f32()) else {
                    error!("Font atlas texture should exist");
                    continue;
                };

                let mut positions = Vec::with_capacity(glyphs.len() * 4);
                let mut uvs = Vec::with_capacity(glyphs.len() * 4);
                let mut colors = Vec::with_capacity(glyphs.len() * 4);
                let mut indices = Vec::with_capacity(glyphs.len() * 6);

                for PositionedGlyph {
                    position,
                    atlas_info,
                    section_index,
                    ..
                } in glyphs
                {
                    let index = positions.len() as u32;
                    let position = (*position + alignment_translation) * Vec2::new(1.0, -1.0);

                    let half_size = atlas_info.rect.size() / 2.0;
                    let top_left = position - half_size;
                    let bottom_right = position + half_size;

                    positions.extend([
                        [top_left.x, top_left.y, 0.0],
                        [top_left.x, bottom_right.y, 0.0],
                        [bottom_right.x, bottom_right.y, 0.0],
                        [bottom_right.x, top_left.y, 0.0],
                    ]);

                    let min = atlas_info.rect.min / atlas_size;
                    let max = atlas_info.rect.max / atlas_size;

                    uvs.extend([
                        [min.x, max.y],
                        [min.x, min.y],
                        [max.x, min.y],
                        [max.x, max.y],
                    ]);

                    let color = section_colors
                        .get(*section_index as usize)
                        .copied()
                        .unwrap_or(LinearRgba::WHITE.to_f32_array());

                    colors.extend([color; 4]);

                    indices.extend([index, index + 2, index + 1, index, index + 3, index + 2]);
                }

                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                );

                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

                mesh.insert_indices(Indices::U32(indices));

                handles.push(BillboardTextHandleGroup {
                    mesh: meshes.add(mesh),
                    image: texture,
                });
            }
        }
    }
}

/// Similar implementation to [`bevy::text::detect_text_needs_rerender`], but checks
/// for changed colors on root text components or child span components.
pub(crate) fn detect_billboard_text_color_change(
    changed_roots: Query<Entity, (Changed<TextColor>, With<BillboardText>)>,
    changed_spans: Query<&ChildOf, (Changed<TextColor>, With<TextSpan>)>,
    mut computed: Query<(
        Entity,
        Option<&ChildOf>,
        Has<ComputedTextBlock>,
        Has<TextSpan>,
    )>,
    mut commands: Commands,
) {
    // TODO: ideally, we can set needs_rerender of `ComputedTextBlock`, but it is
    // private, so we use our own marker component.

    // check for root changes
    for root in &changed_roots {
        let Ok(mut ent) = commands.get_entity(root) else {
            continue;
        };
        ent.insert(BillboardTextNeedsRerender);
    }

    // check for span changes
    for span_parent in &changed_spans {
        let mut parent: Entity = span_parent.0;

        while let Ok((ent, maybe_parent, has_computed, has_span)) = computed.get_mut(parent) {
            if has_computed {
                commands.entity(ent).insert(BillboardTextNeedsRerender);
                break;
            }

            if !has_span {
                break;
            }

            let Some(next_parent) = maybe_parent else {
                break;
            };

            parent = next_parent.0;
        }
    }
}

// TODO: Use EntityHash with EntityHashMap in 0.12 for extracted.
// The related code is removed, but this todo is helpful for future.

#[derive(Component)]
pub struct RenderBillboard {
    pub depth: BillboardDepth,
    pub lock_axis: Option<BillboardLockAxis>,
}
