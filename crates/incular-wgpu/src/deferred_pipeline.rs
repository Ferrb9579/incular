use crate::RendererError;
use std::sync::{Arc, OnceLock};

/// A device/format-owned pipeline compiled once, when first needed by a draw.
/// Clones share both successful compilation and a labeled validation failure.
#[derive(Clone)]
pub(crate) struct DeferredPipeline(Arc<DeferredPipelineInner>);

struct DeferredPipelineInner {
    device: wgpu::Device,
    label: &'static str,
    create: Box<dyn Fn() -> wgpu::RenderPipeline + Send + Sync>,
    value: OnceLock<Result<wgpu::RenderPipeline, String>>,
}

impl DeferredPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &'static str,
        create: impl Fn() -> wgpu::RenderPipeline + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(DeferredPipelineInner {
            device: device.clone(),
            label,
            create: Box::new(create),
            value: OnceLock::new(),
        }))
    }

    pub(crate) fn get(&self) -> Result<&wgpu::RenderPipeline, RendererError> {
        self.0
            .value
            .get_or_init(|| {
                let scope = self
                    .0
                    .device
                    .push_error_scope(wgpu::ErrorFilter::Validation);
                let pipeline = (self.0.create)();
                // Native pipeline creation and validation are synchronous;
                // resolving this error scope does not wait for GPU execution.
                match pollster::block_on(scope.pop()) {
                    Some(error) => Err(error.to_string()),
                    None => Ok(pipeline),
                }
            })
            .as_ref()
            .map_err(|reason| RendererError::PipelineCreation {
                label: self.0.label.to_owned(),
                reason: reason.clone(),
            })
    }

    pub(crate) fn is_created(&self) -> bool {
        matches!(self.0.value.get(), Some(Ok(_)))
    }
}
