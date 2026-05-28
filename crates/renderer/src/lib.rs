mod camera;
mod constellations;
#[cfg(test)]
mod lunar_phase;
mod overlay;
mod pipeline;
mod renderer;
mod skyglow;
mod text;
mod tonemap;
mod vertex;

pub use astronomy::skyglow::LightPollution;
pub use camera::{
    Atmosphere, AtmospherePreset, Camera, ExternalViewpoint, EyepieceSimulation, LocalView,
    Scintillation, SkyProjection, SkyViewpoint,
};
pub use overlay::{OverlayConfig, OverlayKind, DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT};
pub use renderer::Renderer;
pub use vertex::{
    build_star_instance, magnitude_to_render_params, RenderParams, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE, NAKED_EYE_LIMITING_MAGNITUDE, SHADER_INTENSITY_CUTOFF,
    STAR_QUAD_HALF_PX,
};
