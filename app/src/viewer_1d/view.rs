use waragraph_core::graph::Bp;

#[derive(Debug, Clone, PartialEq)]
pub struct View1D {
    left: f64,
    right: f64,
    range: std::ops::Range<u64>,
    max: u64,
}

impl View1D {
    pub fn new(max: u64) -> Self {
        let range = 0..max;
        Self {
            left: 0.0,
            right: max as f64,
            range,
            max,
        }
    }

    pub fn range(&self) -> &std::ops::Range<u64> {
        &self.range
    }

    pub fn offset(&self) -> u64 {
        self.range.start
    }

    pub fn len(&self) -> u64 {
        self.range.end - self.range.start
    }

    pub fn offset_f64(&self) -> f64 {
        self.left
    }

    pub fn len_f64(&self) -> f64 {
        self.right - self.left
    }

    pub fn center_f64(&self) -> f64 {
        self.left + self.len_f64() / 2.0
    }

    pub fn bp_at_norm_f64(&self, t: f64) -> f64 {
        self.left + t.clamp(0.0, 1.0) * self.len_f64()
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn reset(&mut self) {
        self.left = 0.0;
        self.right = self.max as f64;
        self.sync_integer_range();
    }

    fn make_valid(&mut self) {
        let max = self.max as f64;
        let max_len = max.max(1.0);
        let mut len = self.len_f64().clamp(1.0, max_len);

        if !len.is_finite() {
            len = max_len;
        }

        if !self.left.is_finite() {
            self.left = 0.0;
        }

        self.left = self.left.clamp(0.0, (max - len).max(0.0));
        self.right = self.left + len;

        self.sync_integer_range();
    }

    fn sync_integer_range(&mut self) {
        let max = self.max();
        let start = self.left.floor().clamp(0.0, max as f64) as u64;
        let mut end = self.right.ceil().clamp(0.0, max as f64) as u64;

        if end <= start && max > 0 {
            end = (start + 1).min(max);
        }

        self.range = start..end;
    }

    pub fn set(&mut self, left: u64, right: u64) {
        self.left = left as f64;
        self.right = right as f64;
        self.make_valid();
    }

    pub fn translate(&mut self, delta: i64) {
        self.translate_f64(delta as f64);
    }

    pub fn translate_f64(&mut self, delta: f64) {
        self.left += delta;
        self.right += delta;
        self.make_valid();
    }

    /// `delta` is in "view width" units, so +1 means panning the view
    /// to the right by `self.len()` units.
    pub fn translate_norm_f32(&mut self, fdelta: f32) {
        self.translate_f64(fdelta as f64 * self.len_f64());
    }

    /// `fix` is a normalized point in the view [0..1] that will not
    /// move during the zoom
    pub fn zoom_around_norm_f32(&mut self, fix: f32, zdelta: f32) {
        self.zoom_with_focus(fix, zdelta);
    }

    /// Expands/contracts the view by a factor of `s`, keeping the point
    /// corresponding to `t` fixed in the view.
    ///
    /// `t` should be in `[0, 1]`, if `s` > 1.0, the view is zoomed out,
    /// if `s` < 1.0, it is zoomed in.
    pub fn zoom_with_focus(&mut self, t: f32, s: f32) {
        self.zoom_with_focus_f64(t as f64, s as f64);
    }

    pub fn zoom_with_focus_f64(&mut self, t: f64, s: f64) {
        if !s.is_finite() || s <= 0.0 {
            return;
        }

        let t = t.clamp(0.0, 1.0);
        let old_len = self.len_f64();
        let focus = self.bp_at_norm_f64(t);

        let max_len = (self.max as f64).max(1.0);
        let new_len = (old_len * s).clamp(1.0, max_len);

        let mut left = focus - t * new_len;
        let max_left = (self.max as f64 - new_len).max(0.0);
        left = left.clamp(0.0, max_left);

        self.left = left;
        self.right = left + new_len;
        self.sync_integer_range();
    }
}

// various useful view-related transformations
impl View1D {
    // to implement:
    // View1D & Screen space range (e.g. slots)
    //   => Bp -> Screen X;
    //   => Screen X -> Bp

    // View1D & View1D, treated as the view at t = 0 and t = 1 respectively (e.g. transforms)
    //   => Bp -> Bp
    //   => Screen space range -> Bp -> Bp

    /// Maps the view (`self`) to `screen_interval`, and then returns
    /// the intersection of the image of `pan_range` under this map
    /// with the given `screen_interval`. Returns `None` if the
    /// intersection is empty.
    pub fn map_bp_interval_to_screen_x(
        &self,
        pan_range: &std::ops::Range<Bp>,
        screen_interval: &std::ops::RangeInclusive<f32>,
    ) -> Option<std::ops::RangeInclusive<f32>> {
        let vrange = self.range();
        let pleft = pan_range.start.0;
        let pright = pan_range.end.0;

        if pleft > vrange.end || pright < vrange.start {
            return None;
        }

        let vl = vrange.start as f32;
        let vr = vrange.end as f32;
        let vlen = vr - vl;

        let left = pleft.max(vrange.start);
        let right = pright.min(vrange.end);

        let l = left as f32;
        let r = right as f32;

        let lt = (l - vl) / vlen;
        let rt = (r - vl) / vlen;

        let (sleft, sright) = screen_interval.clone().into_inner();
        let slen = sright - sleft;

        let a_left = sleft + lt * slen;
        let a_right = sleft + rt * slen;

        Some(a_left..=a_right)
    }
}

impl View1D {
    pub fn try_center(&mut self, on: std::ops::Range<Bp>) {
        let range_len = on.end.0 - on.start.0;

        let mid = on.start.0 + range_len / 2;

        let cur_mid = self.center_f64();

        if range_len > self.len() {
            // if `on` is bigger than the current view, make the new view
            // some fixed multiple of the input range in size, centered
            // correctly
            self.set(on.start.0, on.end.0);
            self.zoom_around_norm_f32(0.5, 1.5);
        } else {
            // otherwise, do not resize the view, just translate it (if possible)
            let delta = mid as f64 - cur_mid;
            self.translate_f64(delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::View1D;

    #[test]
    fn zoom_with_focus_keeps_off_center_graph_coordinate_anchored() {
        let mut view = View1D::new(10_000);
        view.set(1_000, 5_000);

        let pointer_t = 0.27;
        let before = view.bp_at_norm_f64(pointer_t);

        view.zoom_with_focus_f64(pointer_t, 0.62);

        let after = view.bp_at_norm_f64(pointer_t);
        assert!(
            (before - after).abs() < 1e-6,
            "expected focus {before} to remain anchored, got {after}"
        );
    }

    #[test]
    fn repeated_zoom_in_out_at_same_pointer_does_not_drift_center() {
        let mut view = View1D::new(1_000_000);
        view.set(123_456, 654_321);

        let pointer_t = 0.73;
        let initial_center = view.center_f64();
        let initial_focus = view.bp_at_norm_f64(pointer_t);
        let zoom = 0.875;

        for _ in 0..200 {
            view.zoom_with_focus_f64(pointer_t, zoom);
            view.zoom_with_focus_f64(pointer_t, 1.0 / zoom);
        }

        let center_drift = (view.center_f64() - initial_center).abs();
        let focus_drift =
            (view.bp_at_norm_f64(pointer_t) - initial_focus).abs();

        assert!(
            center_drift < 1e-5,
            "center drifted by {center_drift} after repeated zoom cycles"
        );
        assert!(
            focus_drift < 1e-5,
            "focus drifted by {focus_drift} after repeated zoom cycles"
        );
    }
}
