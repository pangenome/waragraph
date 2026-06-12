use std::sync::Arc;

use async_trait::async_trait;

use anyhow::Result;

use waragraph_core::graph::{
    sampling::proportional_bin_range, Bp, PathId, PathIndex,
};

use crate::app::resource::GraphDataCache;

// pub trait Sampler {
//     fn sample_range_into_bins(&self,
//                               bin_count: usize,
//                               path: PathId,
//                               view: std::ops::Range<Bp>,
//                               ) -> Vec<

// }

// pub struct SamplerMean {
//
// }

#[async_trait]
pub trait Sampler: Send + Sync {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>>;
}

pub struct PathDataSampler {
    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
    data_key: Arc<String>,
}

impl PathDataSampler {
    pub fn new(
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        data_key: &str,
    ) -> Self {
        Self {
            path_index,
            data_cache,
            data_key: Arc::new(data_key.to_string()),
        }
    }
}

#[async_trait]
impl Sampler for PathDataSampler {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let data = self
            .data_cache
            .fetch_path_data(&self.data_key, path)
            .await?;

        let path_index = self.path_index.clone();

        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4 * bin_count];

            let l = view.start.0;
            let r = view.end.0;
            let view_len = (r - l) as usize;
            let used_bins = view_len.min(bin_count);
            let used_slice = &mut buf[..used_bins * 4];

            waragraph_core::graph::sampling::sample_data_into_buffer(
                &path_index,
                path,
                &data.path_data,
                l..r,
                bytemuck::cast_slice_mut(used_slice),
            );

            buf
        })
        .await?;

        Ok(sample_vec)
    }
}

pub struct PathNodeSetSampler {
    path_index: Arc<PathIndex>,
    map: Arc<dyn Fn(PathId, u32) -> f32 + Send + Sync + 'static>,
}

impl PathNodeSetSampler {
    pub fn new(
        path_index: Arc<PathIndex>,
        map: impl Fn(PathId, u32) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self {
            path_index,
            map: Arc::new(map),
        }
    }
}

#[async_trait]
impl Sampler for PathNodeSetSampler {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let path_index = self.path_index.clone();
        let map = self.map.clone();

        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4 * bin_count];
            let l = view.start.0;
            let r = view.end.0;
            let view_len = (r - l) as usize;
            let used_bins = view_len.min(bin_count);
            let used_slice = &mut buf[..used_bins * 4];

            let bins: &mut [f32] = bytemuck::cast_slice_mut(used_slice);

            let path_nodes = &path_index.path_node_sets[path.ix()];

            for (bin_ix, buf_val) in bins.into_iter().enumerate() {
                // pangenome space
                let range = proportional_bin_range(
                    view.start.0..view.end.0,
                    used_bins,
                    bin_ix,
                );

                // get range of nodes corresponding to the pangenome `range`
                let (start, end) =
                    path_index.pos_range_nodes(range).into_inner();
                let ix_range = (start.ix() as u32)..(end.ix() as u32 + 1);

                if path_nodes.range_cardinality(ix_range) > 0 {
                    *buf_val = map(path, 1);
                } else {
                    *buf_val = std::f32::NEG_INFINITY;
                }
            }

            buf
        })
        .await?;

        Ok(sample_vec)
    }
}
