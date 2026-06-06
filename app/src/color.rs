use std::collections::HashMap;
use std::sync::Arc;

use bimap::BiBTreeMap;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BufferUsages,
};

pub mod util;
pub mod widget;

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct ColorMap {
    pub value_range: [f32; 2],
    pub color_range: [f32; 2],
}

/// Sentinel stored in `ColorMap::value_range` to request gfalook `-m` depth
/// binning in shaders instead of normalized color-ramp interpolation.
pub const GFALOOK_DEPTH_VALUE_RANGE: [f32; 2] = [-1.0, 12.5];

/// `~/gfalook/src/main.rs` `COLORBREWER_SPECTRAL_13` and `get_depth_color`
/// default `-m` mapping: two grey low-coverage bins followed by reversed
/// ColorBrewer Spectral 11. Each color covers one depth unit at cuts
/// 0.5, 1.5, 2.5, ..., 12.5; values above 12.5 clamp to the final color.
pub const GFALOOK_DEPTH_PALETTE_RGB: [(u8, u8, u8); 13] = [
    (196, 196, 196),
    (128, 128, 128),
    (158, 1, 66),
    (213, 62, 79),
    (244, 109, 67),
    (253, 174, 97),
    (254, 224, 139),
    (255, 255, 191),
    (230, 245, 152),
    (171, 221, 164),
    (102, 194, 165),
    (50, 136, 189),
    (94, 79, 162),
];

pub fn gfalook_depth_color_index(mean_depth: f32) -> usize {
    for i in 0..GFALOOK_DEPTH_PALETTE_RGB.len() {
        if mean_depth <= 0.5 + i as f32 {
            return i;
        }
    }

    GFALOOK_DEPTH_PALETTE_RGB.len() - 1
}

pub fn gfalook_depth_color_rgb(mean_depth: f32) -> (u8, u8, u8) {
    GFALOOK_DEPTH_PALETTE_RGB[gfalook_depth_color_index(mean_depth)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColorSchemeId(usize);

pub struct ColorStore {
    scheme_name_map: BiBTreeMap<String, ColorSchemeId>,
    color_schemes: Vec<ColorScheme>,

    scheme_buffers: HashMap<ColorSchemeId, Arc<wgpu::Buffer>>,

    scheme_textures:
        HashMap<ColorSchemeId, Arc<(wgpu::Texture, wgpu::TextureView)>>,

    // egui_textures: HashMap<ColorSchemeId, egui::TextureId>,
    pub linear_sampler: Arc<wgpu::Sampler>,
    pub nearest_sampler: Arc<wgpu::Sampler>,
}

fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    let address_mode = wgpu::AddressMode::ClampToEdge;

    let sampler_desc = wgpu::SamplerDescriptor {
        label: Some("Texture Sampler - Color Schemes, Linear"),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 1.0,
        lod_max_clamp: 1.0,
        compare: None,
        // anisotropy_clamp: 0,
        border_color: None,
        ..Default::default()
    };

    device.create_sampler(&sampler_desc)
}

fn create_nearest_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    let address_mode = wgpu::AddressMode::ClampToEdge;

    let sampler_desc = wgpu::SamplerDescriptor {
        label: Some("Texture Sampler - Color Schemes, Nearest"),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 1.0,
        lod_max_clamp: 1.0,
        compare: None,
        border_color: None,
        ..Default::default()
    };

    device.create_sampler(&sampler_desc)
}

impl ColorStore {
    pub fn get_color_scheme_id(&self, name: &str) -> Option<ColorSchemeId> {
        self.scheme_name_map.get_by_left(name).copied()
    }

    pub fn get_scheme_name(&self, id: ColorSchemeId) -> &str {
        // unwrap is fine here since ColorSchemeIds can only be created
        // by adding a color scheme
        self.scheme_name_map.get_by_right(&id).unwrap()
    }

    pub fn get_color_scheme(&self, id: ColorSchemeId) -> &ColorScheme {
        &self.color_schemes[id.0]
    }

    pub fn init(state: &raving_wgpu::State) -> Self {
        let linear_sampler = Arc::new(create_linear_sampler(&state.device));
        let nearest_sampler = Arc::new(create_nearest_sampler(&state.device));

        let mut result = Self {
            scheme_name_map: BiBTreeMap::default(),
            color_schemes: Vec::new(),

            scheme_buffers: HashMap::default(),
            scheme_textures: HashMap::default(),

            linear_sampler,
            nearest_sampler,
            // egui_textures: HashMap::default(),
        };

        let rgba = |r: u8, g: u8, b: u8| {
            let max = u8::MAX as f32;
            [r as f32 / max, g as f32 / max, b as f32 / max, 1.0]
        };

        let spectral = GFALOOK_DEPTH_PALETTE_RGB
            .iter()
            .map(|&(r, g, b)| rgba(r, g, b));

        result.add_color_scheme("spectral", spectral);

        let black_red = (0..8).map(|i: i32| {
            // for i = 8 this is 255, which is what we want
            let r = ((i * 64 - 1) / 2).max(0);
            rgba(r as u8, 0, 0)
        });

        result.add_color_scheme("black_red", black_red);

        result
    }

    pub fn create_color_scheme_texture(
        &mut self,
        state: &raving_wgpu::State,
        scheme_name: &str,
    ) {
        // create texture & texture view
        let scheme_id = *self.scheme_name_map.get_by_left(scheme_name).unwrap();

        let color_scheme = &self.color_schemes[scheme_id.0];

        let dimension = wgpu::TextureDimension::D1;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        let label = format!("Texture - Color Scheme {scheme_name}");

        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST;

        let pixel_data: Vec<_> = color_scheme
            .colors
            .iter()
            .map(|&[r, g, b, a]| {
                [
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    (a * 255.0) as u8,
                ]
            })
            .collect();

        let width = color_scheme.colors.len() as u32;

        let size = wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture_desc = wgpu::TextureDescriptor {
            label: Some(&label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        };

        let texture = state.device.create_texture_with_data(
            &state.queue,
            &texture_desc,
            bytemuck::cast_slice(&pixel_data),
        );

        let label = format!("Texture View - Color Scheme {scheme_name}");

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&label),
            format: Some(format),
            dimension: Some(wgpu::TextureViewDimension::D1),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        self.scheme_textures
            .insert(scheme_id, Arc::new((texture, view)));
    }

    pub fn get_color_scheme_texture(
        &self,
        scheme: ColorSchemeId,
    ) -> Option<Arc<(wgpu::Texture, wgpu::TextureView)>> {
        self.scheme_textures.get(&scheme).cloned()
    }

    pub fn get_color_scheme_gpu_buffer(
        &self,
        id: ColorSchemeId,
    ) -> Option<Arc<wgpu::Buffer>> {
        let buf = self.scheme_buffers.get(&id)?;
        Some(buf.clone())
    }

    pub fn upload_color_schemes_to_gpu(
        &mut self,
        state: &raving_wgpu::State,
    ) -> anyhow::Result<()> {
        let mut need_upload = Vec::new();

        for (ix, _scheme) in self.color_schemes.iter().enumerate() {
            let id = ColorSchemeId(ix);
            if !self.scheme_buffers.contains_key(&id) {
                need_upload.push(id);
            }
        }

        let mut data: Vec<u8> = Vec::new();

        let buffer_usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;

        for id in need_upload {
            data.clear();
            let scheme = self.color_schemes.get(id.0).unwrap();
            data.resize(scheme.required_buffer_size(), 0u8);
            scheme.fill_buffer(&mut data);

            let buffer =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: data.as_slice(),
                    usage: buffer_usage,
                });

            self.scheme_buffers.insert(id, Arc::new(buffer));
        }

        Ok(())
    }

    pub fn add_color_scheme(
        &mut self,
        name: &str,
        colors: impl IntoIterator<Item = [f32; 4]>,
    ) -> ColorSchemeId {
        let id = ColorSchemeId(self.color_schemes.len());

        let scheme = ColorScheme {
            id,
            colors: colors.into_iter().collect(),
        };

        self.scheme_name_map.insert(name.to_string(), id);
        self.color_schemes.push(scheme);

        id
    }
}

/// A `ColorScheme` is a sequence of colors
pub struct ColorScheme {
    pub id: ColorSchemeId,
    pub colors: Vec<[f32; 4]>,
}

impl ColorScheme {
    pub fn required_buffer_size(&self) -> usize {
        let elem_count = self.colors.len();
        let elem_size = std::mem::size_of::<[f32; 4]>();

        // the uniform itself only has a single u32 before the colors,
        // but we need to pad to get the alignment correct
        let prefix_size = std::mem::size_of::<u32>() * 4;

        prefix_size + elem_count * elem_size
    }

    fn fill_buffer(&self, buf: &mut [u8]) {
        assert!(buf.len() >= self.required_buffer_size());

        let len = self.colors.len() as u32;

        let data_start = 4 * 4;

        let data_end =
            data_start + self.colors.len() * std::mem::size_of::<[f32; 4]>();

        buf[0..data_start]
            .clone_from_slice(bytemuck::cast_slice(&[len, 0, 0, 0]));
        buf[data_start..data_end]
            .clone_from_slice(bytemuck::cast_slice(&self.colors));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        gfalook_depth_color_index, gfalook_depth_color_rgb,
        GFALOOK_DEPTH_PALETTE_RGB,
    };

    #[test]
    fn gfalook_depth_low_coverage_uses_explicit_greys() {
        assert_eq!(gfalook_depth_color_rgb(0.5), (196, 196, 196));
        assert_eq!(gfalook_depth_color_rgb(1.0), (128, 128, 128));
        assert_eq!(gfalook_depth_color_rgb(1.5), (128, 128, 128));
    }

    #[test]
    fn gfalook_depth_higher_coverage_uses_reversed_spectral_bins() {
        assert_eq!(gfalook_depth_color_rgb(2.0), (158, 1, 66));
        assert_eq!(gfalook_depth_color_rgb(3.0), (213, 62, 79));
        assert_eq!(gfalook_depth_color_rgb(12.0), (94, 79, 162));
        assert_eq!(gfalook_depth_color_rgb(100.0), (94, 79, 162));
    }

    #[test]
    fn gfalook_depth_bins_are_equivalent_to_gfalook_m_cuts() {
        let cases = [
            (0.0, 0),
            (0.5, 0),
            (0.5001, 1),
            (1.5, 1),
            (1.5001, 2),
            (2.5, 2),
            (2.5001, 3),
            (12.5, 12),
            (12.5001, 12),
        ];

        for (depth, expected_index) in cases {
            assert_eq!(gfalook_depth_color_index(depth), expected_index);
            assert_eq!(
                gfalook_depth_color_rgb(depth),
                GFALOOK_DEPTH_PALETTE_RGB[expected_index]
            );
        }
    }
}
