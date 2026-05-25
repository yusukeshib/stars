mod camera;
mod constellations;
mod overlay;
mod pipeline;
mod renderer;
mod skyglow;
mod tonemap;
mod vertex;

pub use camera::{Atmosphere, AtmospherePreset, Camera, LocalView, SkyProjection};
pub use overlay::{OverlayConfig, OverlayKind};
pub use renderer::Renderer;
pub use vertex::{
    build_star_instance, magnitude_to_render_params, RenderParams, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE, NAKED_EYE_LIMITING_MAGNITUDE, SHADER_INTENSITY_CUTOFF,
    STAR_QUAD_HALF_PX,
};
