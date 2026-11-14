use std::{hash::{DefaultHasher, Hash, Hasher}, marker::PhantomData, num::NonZero};

use async_store::HoldPolicy;
use uuid::Uuid;

use crate::context::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HashableSamplerDescriptor {
    pub label: &'static str,
    pub address_mode: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,
    pub mipmap_filter: wgpu::MipmapFilterMode,
    /// used to convert lod clamps into floats for wgpu
    pub lod_divisor: u32,
    pub lod_min_clamp: u32,
    pub lod_max_clamp: u32, 
    pub anisotropy_clamp: u16,
    pub compare: Option<wgpu::CompareFunction>,
    pub border_color: Option<wgpu::SamplerBorderColor>,
}

impl HashableSamplerDescriptor {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            address_mode: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0,
            lod_max_clamp: 32, 
            anisotropy_clamp: 1,
            compare: None,
            border_color: None,
            lod_divisor: 1,
        }
    }

    /// Set the xyz address mode
    pub fn with_address_mode(mut self, mode: wgpu::AddressMode) -> Self {
        self.address_mode = mode;
        self
    }

    /// set the lod min and max
    pub fn with_lod(mut self, min: u32, max: u32, divisor: u32) -> Self {
        self.lod_min_clamp = min;
        self.lod_max_clamp = max;
        self.lod_divisor = divisor;
        self
    }

    /// set the min/mag/mip filter mode to nearest
    pub fn with_nearest_filtering(mut self) -> Self {
        self.set_nearest_filtering();
        self
    }

    /// set the min/mag/mip filter mode to nearest
    pub fn set_nearest_filtering(&mut self) {
        self.min_filter = wgpu::FilterMode::Nearest;
        self.mag_filter = wgpu::FilterMode::Nearest;
        self.mipmap_filter = wgpu::MipmapFilterMode::Nearest;
    }

    /// Set the anisotrophy clamp
    pub fn with_anisotrophy(mut self, clamp: u16) -> Self {
        self.anisotropy_clamp = clamp;
        self
    }

    /// Set the compare function
    pub fn with_compare(mut self, compare: wgpu::CompareFunction) -> Self {
        self.compare = Some(compare);
        self
    }

    /// Set the border color
    pub fn with_border_color(mut self, color: wgpu::SamplerBorderColor) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Converts `self` into a `wgpu::SamplerDescriptor`
    pub fn into_wgpu_desc(self) -> wgpu::SamplerDescriptor<'static> {
        wgpu::SamplerDescriptor {
            label: Some(self.label),
            address_mode_u: self.address_mode,
            address_mode_v: self.address_mode,
            address_mode_w: self.address_mode,
            min_filter: self.min_filter,
            mag_filter: self.mag_filter,
            mipmap_filter: self.mipmap_filter,
            lod_min_clamp: self.lod_min_clamp as f32 / self.lod_divisor as f32,
            lod_max_clamp: self.lod_max_clamp as f32 / self.lod_divisor as f32, 
            anisotropy_clamp: self.anisotropy_clamp,
            compare: self.compare,
            border_color: self.border_color
        }
    }
}

pub struct SamplerPayload {
    pub desc: wgpu::SamplerDescriptor<'static>,
}

/// Represents resources that can be condensed into a sampler payload
pub trait SamplerType: Send + 'static {
    /// Convert the sampler into its full payload information, if possible.
    fn into_payload(self) -> Result<SamplerPayload, String>;

    /// Returns the hold policy associated with the sampler, if exists.
    fn hold_policy(&self) -> Option<HoldPolicy>;
}

/// A blueprint for constructing samplers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SamplerBlueprint<T: Hash> {
    pub(crate) desc: HashableSamplerDescriptor,
    pub(crate) hold_policy: Option<HoldPolicy>,
    ty: PhantomData<T>
}

impl<T: Hash> SamplerBlueprint<T> {
    /// Set the label for gpu profiling of the resultant texture
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.desc.label = label;
        self
    }

    /// Set the lod clamp range. This panics if `min` > `max`
    /// 
    /// The `min` and `max` values are divided by `lod_divisor` to convert them into floats just before the `wgpu::Sampler` is created. 
    /// This allows the `SamplerBlueprint` to be hashable without complex float hashing algorithms or losing precision.
    /// 
    /// Ex: `min`: 250, `max`: 500, `lod_divisor`: 1000 -> `min`: 0.25, `max`: 0.5
    pub fn with_lod_clamp(mut self, min: u32, max: u32, lod_divisor: u32) -> Self {
        assert!(min <= max, "min lod must be less than or equal to max lod");

        self.desc.lod_min_clamp = min;
        self.desc.lod_max_clamp = max;
        self.desc.lod_divisor = lod_divisor;
        self
    }

    /// Set the hold policy for this sampler. This overrides the global policy on the `GpuContext`.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }
}

impl<T: Hash> GpuResource for SamplerBlueprint<T> {
    type Id = SamplerId;

    fn create_id(&self) -> Self::Id {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        return SamplerId(Uuid::from_u128(hasher.finish() as u128));
    }
}

/// Sampler type for linear filtering
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Linear;
impl SamplerBlueprint<Linear> {
    /// Create a sampler for linear filtering
    pub fn linear() -> Self {
        Self {
            ty: std::marker::PhantomData,
            hold_policy: None,
            desc: HashableSamplerDescriptor::new("linear_sampler")
        }
    }

    /// Set the mipmap filter to nearest. (min and mag filter remain linear)
    pub fn with_mipmap_nearest(mut self) -> Self {
        self.desc.mipmap_filter = wgpu::MipmapFilterMode::Nearest;
        self
    }

    /// Set the address mode to `Repeat`, or `MirrorRepeat` if `mirrored` is set to true.
    pub fn with_repeat(mut self, mirrored: bool) -> Self {
        let mode = if mirrored {
            wgpu::AddressMode::MirrorRepeat
        } else {
            wgpu::AddressMode::Repeat
        };

        self.desc.address_mode = mode;
        self
    }
}

impl SamplerType for SamplerBlueprint<Linear> {
    fn into_payload(self) -> Result<SamplerPayload, String> {
        Ok(SamplerPayload { desc: self.desc.into_wgpu_desc() })
    }

    fn hold_policy(&self) -> Option<HoldPolicy> {
        self.hold_policy
    }
}

/// Sampler type for nearest filtering
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Nearest;
impl SamplerBlueprint<Nearest> {
    /// Create a sampler for nearest filtering.
    /// 
    /// Note: The anisotrophic level is locked to `1`. If you need a higher level, call `SamplerBlueprint::anisotrophic()`
    pub fn nearest() -> Self {
        Self {
            hold_policy: None,
            desc: HashableSamplerDescriptor::new("nearest_sampler")
                .with_nearest_filtering(),
            ty: std::marker::PhantomData,
        }
    }

    /// Set the mipmap filter to linear. (min and mag filter remain nearest)
    pub fn with_mipmap_linear(mut self) -> Self {
        self.desc.mipmap_filter = wgpu::MipmapFilterMode::Linear;
        self
    }
}

impl SamplerType for SamplerBlueprint<Nearest> {
    fn into_payload(self) -> Result<SamplerPayload, String> {
        Ok(SamplerPayload { desc: self.desc.into_wgpu_desc() })
    }

    fn hold_policy(&self) -> Option<HoldPolicy> {
        self.hold_policy
    }
}

/// Sampler type for anisotropic filtering
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Anisotropic;

impl SamplerBlueprint<Anisotropic> {
    /// Create a sampler for anisotropic filtering. Default address mode is `Repeat`
    /// 
    /// Note: the min/mag/mip filters are locked to `Linear`. If you need a `Nearest` filtering sampler, call `SamplerBlueprint::nearest()`
    pub fn anisotropic(level: NonZero<u16>) -> Self {
        Self {
            ty: std::marker::PhantomData,
            hold_policy: None,
            desc: HashableSamplerDescriptor::new("anisotropic_sampler")
                .with_address_mode(wgpu::AddressMode::Repeat)
                .with_anisotrophy(level.get())
        }
    }

    /// Sets the address mode to `MirrorRepeat`
    pub fn mirrored(mut self) -> Self {
        self.desc.address_mode = wgpu::AddressMode::MirrorRepeat;
        self
    }

    /// Sets the address mode to `ClampToEdge`
    pub fn clamp_edge(mut self) -> Self {
        self.desc.address_mode = wgpu::AddressMode::ClampToEdge;
        self
    }
}

impl SamplerType for SamplerBlueprint<Anisotropic> {
    fn into_payload(self) -> Result<SamplerPayload, String> {
        if self.desc.anisotropy_clamp > 16u16 {
            Err(format!("Cannot create sampler with label '{:?}'. Anisotropic level must be in the range [1, 16].", self.desc.label))
        } else {
            Ok(SamplerPayload { desc: self.desc.into_wgpu_desc() })
        }
    }

    fn hold_policy(&self) -> Option<HoldPolicy> {
        self.hold_policy
    }
}

/// Sampler type for borders around sampled texture
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Bordered;

impl SamplerBlueprint<Bordered> {
    /// Create a sampler with address mode `ClampToBorder`.
    /// 
    /// Note: This assumes no comparison function. Instead, you can call `SamplerBlueprint::comparison().with_clamp_border()` to create a comparison sampler with a border color.
    pub fn bordered(color: wgpu::SamplerBorderColor) -> Self {
        Self {
            ty: std::marker::PhantomData,
            hold_policy: None,
            desc: HashableSamplerDescriptor::new("bordered_sampler")
                .with_address_mode(wgpu::AddressMode::ClampToBorder)
                .with_border_color(color)
        }
    }

    /// Sets the min/mag/mip filter to `Nearest`
    pub fn sample_nearest(mut self) -> Self {
        self.desc.set_nearest_filtering();
        self
    }

    /// Set the mipmap filter to `Linear`. (min and mag filter remain what they were previously set - default is `Linear`)
    pub fn with_mipmap_linear(mut self) -> Self {
        self.desc.mipmap_filter = wgpu::MipmapFilterMode::Linear;
        self
    }
}

impl SamplerType for SamplerBlueprint<Bordered> {
    fn into_payload(self) -> Result<SamplerPayload, String> {
        Ok(SamplerPayload { desc: self.desc.into_wgpu_desc() })
    }

    fn hold_policy(&self) -> Option<HoldPolicy> {
        self.hold_policy
    }
}

/// Sampler type for comparison between values in sampled texture
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Comparison;

impl SamplerBlueprint<Comparison> {
    /// Create a sampler for value comparisons.
    pub fn comparison(func: wgpu::CompareFunction) -> Self {
        Self {
            ty: std::marker::PhantomData,
            hold_policy: None,
            desc: HashableSamplerDescriptor::new("compare_sampler")
                .with_compare(func)
        }
    }

    /// Sets the min/mag/mip filter to `Nearest`
    pub fn sample_nearest(mut self) -> Self {
        self.desc.set_nearest_filtering();
        self
    }

    /// Set the mipmap filter to `Linear`. (min and mag filter remain what they were previously set - default is `Linear`)
    pub fn with_mipmap_linear(mut self) -> Self {
        self.desc.mipmap_filter = wgpu::MipmapFilterMode::Linear;
        self
    }

    /// Set the address mode to `ClampToBorder` with the specified color
    pub fn with_clamp_border(mut self, color: wgpu::SamplerBorderColor) -> Self {
        self.desc.border_color = Some(color);
        self.desc.address_mode = wgpu::AddressMode::ClampToBorder;
        self
    }
}

impl SamplerType for SamplerBlueprint<Comparison> {
    fn into_payload(self) -> Result<SamplerPayload, String> {
        Ok(SamplerPayload { desc: self.desc.into_wgpu_desc() })
    }

    fn hold_policy(&self) -> Option<HoldPolicy> {
        self.hold_policy
    }
}