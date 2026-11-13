use bevy::{math::Mat4, transform::components::GlobalTransform};

use crate::{pipeline::BillboardUniform, BillboardLockAxis};

/// Builds the uniform matrix for camera-facing billboards.
///
/// The shader only reads the X/Y diagonal (scale, applied along the camera's right/up
/// vectors) and `w_axis` (translation). Z scale is intentionally ignored: billboards are
/// flat, and applying it per world axis would squash them depending on view direction.
pub fn compute_matrix_without_rotation(global_transform: &GlobalTransform) -> Mat4 {
    let global_matrix = global_transform.to_matrix();
    let (global_scale, _, _) = global_matrix.to_scale_rotation_translation();
    Mat4::from_cols(
        Mat4::IDENTITY.x_axis * global_scale.x,
        Mat4::IDENTITY.y_axis * global_scale.y,
        Mat4::IDENTITY.z_axis,
        global_matrix.w_axis,
    )
}

pub fn calculate_billboard_uniform(
    global_transform: &GlobalTransform,
    lock_axis: Option<&BillboardLockAxis>,
) -> BillboardUniform {
    // Unless rotation is locked, we only need scale + translation.
    let transform = if lock_axis.is_some_and(|lock| lock.rotation) {
        global_transform.to_matrix()
    } else {
        compute_matrix_without_rotation(global_transform)
    };

    BillboardUniform { transform }
}
