use std::hash::Hash;

use async_store::HoldPolicy;
use uuid::Uuid;

use crate::{GpuResource, ShaderId};

/// The method of obtaining the source code for a shader module
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ShaderSource {
    /// Shader code is loaded in dynamically from disk
    OnDisk(String),
    /// Shader code is embedded in the binary of the driver program
    /// 
    /// Works best with `include_str!()`
    Embedded(String)
}

/// A blueprint for constructing shader modules
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ShaderBlueprint {
    pub(crate) source: ShaderSource,
    pub(crate) hold_policy: Option<HoldPolicy>,
    pub(crate) label: String,
}

impl ShaderBlueprint {
    /// Create a shader blueprint for shader code stored on disk
    pub fn on_disk(path: impl Into<String>) -> Self {
        Self {
            source: ShaderSource::OnDisk(path.into()),
            hold_policy: None,
            label: "shader_from_disk".to_string()
        }
    }

    /// Create a shader blueprint for shader code embedded in the driver binary.
    /// 
    /// This works best with `include_str!()`
    pub fn embedded(source: impl Into<String>) -> Self {
        Self {
            source: ShaderSource::Embedded(source.into()),
            hold_policy: None,
            label: "shader_from_disk".to_string()
        }
    }

    /// Set the label for gpu profiling of the resultant render pipeline
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Set the hold policy for this compute pipeline. This overrides the global policy.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }
}

impl GpuResource for ShaderBlueprint {
    type Id = ShaderId;

    fn create_id(&self) -> Self::Id {
        ShaderId(Uuid::new_v4())
    }
}