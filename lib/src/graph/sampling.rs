use std::{collections::BTreeMap, ops::Range};

use super::{Node, PathId, PathIndex};

pub trait PathData<T> {
    fn get_path(&self, path_id: PathId) -> &[T];
}

pub fn proportional_bin_range(
    view_range: Range<u64>,
    bin_count: usize,
    bin_ix: usize,
) -> Range<u64> {
    assert!(bin_count > 0, "bin_count must be non-zero");
    assert!(bin_ix < bin_count, "bin_ix must be less than bin_count");

    let start = view_range.start;
    let len = view_range.end.saturating_sub(view_range.start) as u128;
    let bin_count = bin_count as u128;
    let bin_ix = bin_ix as u128;

    let bin_start = start + ((len * bin_ix) / bin_count) as u64;
    let bin_end = start + ((len * (bin_ix + 1)) / bin_count) as u64;

    bin_start..bin_end
}

pub fn sample_data_into_buffer(
    index: &PathIndex,
    path_id: PathId,
    path_data: &[f32],
    view_range: Range<u64>,
    bins: &mut [f32],
) {
    let bin_count = bins.len();
    bins.fill(f32::NEG_INFINITY);

    if bin_count == 0 || view_range.start >= view_range.end {
        return;
    }

    let view_len = usize::try_from(view_range.end - view_range.start)
        .unwrap_or(usize::MAX);
    let used_bins = view_len.min(bin_count);

    for (bin_ix, buf_val) in bins[..used_bins].iter_mut().enumerate() {
        let range =
            proportional_bin_range(view_range.clone(), used_bins, bin_ix);
        let iter = index.path_data_pan_range_iter(range, path_id, path_data);

        let mut sum_len = 0;
        let mut sum_val = 0.0;

        for ((_node, len), val) in iter {
            sum_len += len.0;
            sum_val += *val * len.0 as f32;
        }

        if sum_len > 0 {
            *buf_val = sum_val / sum_len as f32;
        }
    }
}

pub fn sample_path_data_into_buffer<D>(
    index: &PathIndex,
    data: &D,
    paths: impl IntoIterator<Item = PathId>,
    bins: usize,
    view_range: Range<u64>,
    out: &mut [u8],
) where
    D: PathData<f32>,
{
    let paths = paths.into_iter().collect::<Vec<_>>();

    // the part that holds the row size & total size
    let prefix_size = std::mem::size_of::<u32>() * 4;

    let view_len =
        usize::try_from(view_range.end.saturating_sub(view_range.start))
            .unwrap_or(usize::MAX);
    let bins = view_len.min(bins);

    let elem_size = std::mem::size_of::<f32>();
    let needed_size = prefix_size + elem_size * bins * paths.len();

    // TODO: return Err when `out` has to be reallocated
    assert!(out.len() >= needed_size);

    let row_size = bins;
    let total_size = row_size * paths.len();

    out[0..16].clone_from_slice(bytemuck::cast_slice(&[
        total_size as u32,
        row_size as u32,
        0,
        0,
    ]));

    if bins == 0 {
        return;
    }

    let data_offset = 16;

    let row_size = elem_size * row_size;

    for (ix, path_id) in paths.into_iter().enumerate() {
        let offset = data_offset + ix * row_size;
        let range = offset..(offset + row_size);

        let path_data = data.get_path(path_id);
        let buf_row: &mut [f32] = bytemuck::cast_slice_mut(&mut out[range]);

        for (buf_val, bin_ix) in buf_row.iter_mut().zip(0..bins) {
            // using negative infinity as a marker for empty bins
            *buf_val = f32::NEG_INFINITY;
            let range =
                proportional_bin_range(view_range.clone(), bins, bin_ix);
            let iter =
                index.path_data_pan_range_iter(range, path_id, path_data);

            let mut sum_len = 0;
            let mut sum_val = 0.0;

            for ((_node, len), val) in iter {
                sum_len += len.0;
                sum_val += *val * len.0 as f32;
            }

            if sum_len > 0 {
                *buf_val = sum_val / sum_len as f32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("waragraph-{}-{nanos}-{name}", std::process::id(),))
    }

    #[test]
    fn proportional_bins_cover_entire_view_without_truncating_tail() {
        let view = 10..1798;
        let bin_count = 1024;

        let mut previous_end = view.start;
        let mut covered = 0;

        for bin_ix in 0..bin_count {
            let range = proportional_bin_range(view.clone(), bin_count, bin_ix);
            assert_eq!(previous_end, range.start);
            assert!(range.start < range.end);
            covered += range.end - range.start;
            previous_end = range.end;
        }

        assert_eq!(view.end, previous_end);
        assert_eq!(view.end - view.start, covered);
    }

    #[test]
    fn proportional_bins_distribute_remainder() {
        let bins = (0..3)
            .map(|bin_ix| proportional_bin_range(0..10, 3, bin_ix))
            .collect::<Vec<_>>();

        assert_eq!(bins, vec![0..3, 3..6, 6..10]);
    }

    #[test]
    fn sample_data_into_buffer_uses_all_of_non_divisible_view() {
        let path = temp_path("non-divisible-sampling.gfa");
        let mut gfa = String::from("H\tVN:Z:1.0\n");
        for node in 1..=10 {
            gfa.push_str(&format!("S\t{node}\tA\n"));
        }
        let steps = (1..=10)
            .map(|node| format!("{node}+"))
            .collect::<Vec<_>>()
            .join(",");
        gfa.push_str(&format!("P\tp\t{steps}\t*\n"));

        std::fs::write(&path, gfa).unwrap();
        let index = PathIndex::from_gfa(&path).unwrap();
        let _ = std::fs::remove_file(path);

        let path_data = (1..=10).map(|v| v as f32).collect::<Vec<_>>();
        let mut bins = vec![0.0; 6];

        sample_data_into_buffer(
            &index,
            PathId::from(0usize),
            &path_data,
            0..10,
            &mut bins,
        );

        assert_eq!(bins, vec![1.0, 2.5, 4.5, 6.0, 7.5, 9.5]);
    }
}

pub struct PathDepthData {
    pub node_depth_per_path: Vec<Vec<f32>>,
}

impl PathData<f32> for PathDepthData {
    fn get_path(&self, path_id: PathId) -> &[f32] {
        &self.node_depth_per_path[path_id.ix()]
    }
}

impl PathDepthData {
    pub fn new(path_index: &PathIndex) -> Self {
        let mut data = Vec::new();

        for (path_id, _node_set) in path_index.path_node_sets.iter().enumerate()
        {
            let mut path_data: BTreeMap<Node, f32> = BTreeMap::default();
            for step in path_index.path_steps[path_id].iter() {
                *path_data.entry(step.node()).or_default() += 1.0;
            }
            let path_data =
                path_data.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
            data.push(path_data);
        }

        Self {
            node_depth_per_path: data,
        }
    }
}
