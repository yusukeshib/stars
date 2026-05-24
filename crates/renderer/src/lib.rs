mod camera;
mod overlay;
mod pipeline;
mod renderer;
mod vertex;

pub use camera::{Camera, CameraUniform, LocalView};
pub use overlay::{OverlayConfig, OverlayKind};
pub use renderer::Renderer;
pub use vertex::{
    magnitude_to_render_params, QuadVertex, RenderParams, StarInstance,
    NAKED_EYE_LIMITING_MAGNITUDE, SHADER_INTENSITY_CUTOFF,
};
