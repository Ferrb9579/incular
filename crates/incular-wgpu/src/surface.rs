//! Native presentation-surface policy.
//!
//! This module translates platform-level window transparency into WGPU's
//! explicit compositor contract. It does not derive presentation behavior from
//! draw colors: surface compositing is a window property, while alpha in render
//! content is valid for antialiasing, effects, and offscreen composition in
//! every window.

use incular_config::TransparencyMode;
use incular_core::Color;

/// Alpha representation expected in the texture handed to the native
/// compositor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceAlphaRepresentation {
    /// Framebuffer alpha is ignored by the native compositor. Incular's render
    /// target still contains premultiplied scene RGB because ordinary
    /// source-over blending accumulates that representation.
    Opaque,
    /// RGB is already multiplied by alpha before presentation.
    Premultiplied,
    /// RGB is straight; the native compositor multiplies it by alpha.
    Straight,
}

impl SurfaceAlphaRepresentation {
    /// Converts one RGBA8 surface readback pixel into Incular's canonical
    /// straight-alpha RGBA8 representation.
    ///
    /// The input is already in RGBA channel order. For sRGB targets the RGB
    /// channels are decoded to linear light before unpremultiplication and
    /// encoded again afterwards. Dividing encoded sRGB bytes by alpha would be
    /// mathematically incorrect and shifts partially transparent colors.
    #[must_use]
    pub fn to_straight_rgba8(self, format: wgpu::TextureFormat, rgba: [u8; 4]) -> [u8; 4] {
        if matches!(self, Self::Straight) || rgba[3] == u8::MAX {
            return rgba;
        }
        if rgba[3] == 0 {
            return [0; 4];
        }

        let alpha = f32::from(rgba[3]) / 255.0;
        if format.is_srgb() {
            let [red, green, blue, _] =
                Color::rgba(rgba[0], rgba[1], rgba[2], rgba[3]).to_linear_rgba();
            let straight =
                Color::from_linear_rgba([red / alpha, green / alpha, blue / alpha, alpha]);
            [straight.red, straight.green, straight.blue, straight.alpha]
        } else {
            let unpremultiply = |channel: u8| -> u8 {
                ((f32::from(channel) / 255.0 / alpha).clamp(0.0, 1.0) * 255.0).round() as u8
            };
            [
                unpremultiply(rgba[0]),
                unpremultiply(rgba[1]),
                unpremultiply(rgba[2]),
                rgba[3],
            ]
        }
    }
}

/// Concrete presentation contract selected for one WGPU surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceAlphaPlan {
    composite_mode: wgpu::CompositeAlphaMode,
    representation: SurfaceAlphaRepresentation,
}

impl SurfaceAlphaPlan {
    /// Resolves a window-level transparency contract against one surface's
    /// advertised WGPU alpha capabilities.
    pub fn select(
        transparency_mode: TransparencyMode,
        supported: &[wgpu::CompositeAlphaMode],
    ) -> Result<Self, SurfaceAlphaError> {
        if transparency_mode == TransparencyMode::Transparent {
            if supported.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                return Ok(Self {
                    composite_mode: wgpu::CompositeAlphaMode::PreMultiplied,
                    representation: SurfaceAlphaRepresentation::Premultiplied,
                });
            }
            if supported.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                return Ok(Self {
                    composite_mode: wgpu::CompositeAlphaMode::PostMultiplied,
                    representation: SurfaceAlphaRepresentation::Straight,
                });
            }
            return Err(SurfaceAlphaError::TransparentCompositingUnsupported);
        }
        if supported.contains(&wgpu::CompositeAlphaMode::Opaque) {
            return Ok(Self {
                composite_mode: wgpu::CompositeAlphaMode::Opaque,
                representation: SurfaceAlphaRepresentation::Opaque,
            });
        }
        if supported.contains(&wgpu::CompositeAlphaMode::Inherit) {
            return Ok(Self {
                composite_mode: wgpu::CompositeAlphaMode::Inherit,
                representation: SurfaceAlphaRepresentation::Opaque,
            });
        }
        Err(SurfaceAlphaError::NoOpaqueCompositingMode)
    }

    #[must_use]
    pub const fn composite_mode(self) -> wgpu::CompositeAlphaMode {
        self.composite_mode
    }

    #[must_use]
    pub const fn representation(self) -> SurfaceAlphaRepresentation {
        self.representation
    }

    #[must_use]
    pub const fn requires_straight_alpha_conversion(self) -> bool {
        matches!(self.representation, SurfaceAlphaRepresentation::Straight)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceAlphaError {
    TransparentCompositingUnsupported,
    NoOpaqueCompositingMode,
}

impl std::fmt::Display for SurfaceAlphaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransparentCompositingUnsupported => formatter.write_str(
                "transparent window requires premultiplied or postmultiplied surface compositing, but the surface supports neither",
            ),
            Self::NoOpaqueCompositingMode => formatter.write_str(
                "surface reported neither opaque nor inherited compositing for an opaque window",
            ),
        }
    }
}

impl std::error::Error for SurfaceAlphaError {}
