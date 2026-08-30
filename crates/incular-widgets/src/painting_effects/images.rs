//! Image-backed painting effect descriptors.

use incular_core::Color;
use incular_image::ImageHandle;
use typed_builder::TypedBuilder;

use crate::tree::ImageFit;
use crate::{Blur, BlurController, Image, ImageRepeat, Widget};

/// Renders a raw raster image buffer directly without asset caching.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct RawImage {
    #[builder(default, setter(strip_option))]
    image: Option<ImageHandle>,
    #[builder(
        default,
        setter(transform = |width: f32| Some(width.max(0.0)))
    )]
    pub(super) width: Option<f32>,
    #[builder(
        default,
        setter(transform = |height: f32| Some(height.max(0.0)))
    )]
    pub(super) height: Option<f32>,
    #[builder(
        default = 1.0,
        setter(transform = |scale: f32| scale.max(0.001))
    )]
    pub(super) scale: f32,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = ImageFit::Contain)]
    pub(super) fit: ImageFit,
    #[builder(default = ImageRepeat::NoRepeat)]
    pub(super) repeat: ImageRepeat,
}

impl Default for RawImage {
    fn default() -> Self {
        Self::new()
    }
}

impl RawImage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            image: None,
            width: None,
            height: None,
            scale: 1.0,
            color: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
        }
    }

    #[must_use]
    pub fn image(mut self, image: ImageHandle) -> Self {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.001);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
}

impl From<RawImage> for Widget {
    fn from(value: RawImage) -> Self {
        match value.image {
            Some(handle) => {
                let mut img = Image::new(handle).fit(value.fit).repeat(value.repeat);
                if let Some(w) = value.width {
                    img = img.width(w);
                }
                if let Some(h) = value.height {
                    img = img.height(h);
                }
                img.into()
            }
            None => crate::layout::SizedBox::new()
                .width(value.width.unwrap_or(0.0))
                .height(value.height.unwrap_or(0.0))
                .into(),
        }
    }
}

/// An icon that comes from an [`ImageHandle`].
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ImageIcon {
    image: ImageHandle,
    #[builder(
        default,
        setter(transform = |size: f32| Some(size.max(0.0)))
    )]
    pub(super) size: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub(super) color: Option<Color>,
}

impl ImageIcon {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            size: None,
            color: None,
        }
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<ImageIcon> for Widget {
    fn from(value: ImageIcon) -> Self {
        let size = value.size.unwrap_or(24.0);
        Image::new(value.image)
            .width(size)
            .height(size)
            .fit(ImageFit::Contain)
            .into()
    }
}

/// An image filter applied directly to its child subtree.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ImageFiltered {
    sigma: f32,
    #[builder(default, setter(skip))]
    pub(super) controller: Option<BlurController>,
    #[builder(setter(into))]
    child: Widget,
}

impl ImageFiltered {
    #[must_use]
    pub fn blur(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma,
            controller: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
}

impl From<ImageFiltered> for Widget {
    fn from(value: ImageFiltered) -> Self {
        match value.controller {
            Some(c) => Blur::controlled(c, value.child).into(),
            None => Blur::new(value.sigma, value.child).into(),
        }
    }
}
