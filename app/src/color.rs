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

impl ColorMap {
    pub fn for_value_range(min: f32, max: f32) -> Self {
        let min = if min.is_finite() { min } else { 0.0 };
        let max = if max.is_finite() { max } else { min };

        Self {
            value_range: [min, max],
            color_range: [0.0, 1.0],
        }
    }

    pub fn normalized_color_position(&self, value: f32) -> f32 {
        let [min_val, max_val] = self.value_range;
        let [min_color, max_color] = self.color_range;
        let range = max_val - min_val;
        let value_t = if range > 0.0 {
            ((value - min_val) / range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        min_color + (max_color - min_color) * value_t
    }
}

/// Path-depth colors for nodes: lowest observed depth is grey, then increasing
/// depth advances through ROYGBIV. The color map range is set from graph depth
/// stats so high-path-count graphs do not collapse to the final violet bin.
pub const PATH_DEPTH_PALETTE_RGB: [(u8, u8, u8); 8] = [
    (128, 128, 128),
    (228, 26, 28),
    (255, 127, 0),
    (255, 255, 51),
    (77, 175, 74),
    (55, 126, 184),
    (75, 0, 130),
    (148, 0, 211),
];

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

        let depth = PATH_DEPTH_PALETTE_RGB
            .iter()
            .map(|&(r, g, b)| rgba(r, g, b));

        result.add_color_scheme("depth", depth);

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
    use super::{ColorMap, PATH_DEPTH_PALETTE_RGB};

    #[test]
    fn path_depth_palette_is_grey_then_roygbiv() {
        assert_eq!(
            PATH_DEPTH_PALETTE_RGB,
            [
                (128, 128, 128),
                (228, 26, 28),
                (255, 127, 0),
                (255, 255, 51),
                (77, 175, 74),
                (55, 126, 184),
                (75, 0, 130),
                (148, 0, 211),
            ]
        );
    }

    #[test]
    fn color_map_uses_data_range_instead_of_fixed_depth_cap() {
        let color_map = ColorMap::for_value_range(0.0, 210.0);

        assert!(color_map.normalized_color_position(13.0) < 0.1);
        assert_eq!(color_map.normalized_color_position(210.0), 1.0);
    }

    #[test]
    fn single_depth_range_maps_to_grey_end() {
        let color_map = ColorMap::for_value_range(42.0, 42.0);

        assert_eq!(color_map.normalized_color_position(42.0), 0.0);
    }
}
