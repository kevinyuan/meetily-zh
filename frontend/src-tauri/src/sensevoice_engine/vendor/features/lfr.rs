use ndarray::Array2;

/// Apply Lower Frame Rate (LFR) stacking.
///
/// Concatenates `window_size` consecutive frames with a stride of `window_shift`,
/// reducing temporal resolution while increasing feature dimension.
///
/// Input shape: [num_frames, feat_dim]
/// Output shape: [(num_frames - window_size) / window_shift + 1, feat_dim * window_size]
pub fn apply_lfr(features: &Array2<f32>, window_size: usize, window_shift: usize) -> Array2<f32> {
    let in_frames = features.nrows();
    let in_dim = features.ncols();

    if in_frames < window_size {
        return Array2::zeros((0, in_dim * window_size));
    }

    let out_frames = (in_frames - window_size) / window_shift + 1;
    let out_dim = in_dim * window_size;

    let mut out = Array2::zeros((out_frames, out_dim));

    for i in 0..out_frames {
        let src_start = i * window_shift;
        for w in 0..window_size {
            let src_row = features.row(src_start + w);
            let dst_start = w * in_dim;
            for (j, &val) in src_row.iter().enumerate() {
                out[[i, dst_start + j]] = val;
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(frames: usize, dim: usize) -> Array2<f32> {
        Array2::from_shape_fn((frames, dim), |(r, c)| (r * 100 + c) as f32)
    }

    #[test]
    fn output_shape_follows_the_documented_formula() {
        // SenseVoice uses window 7, shift 6.
        let out = apply_lfr(&ramp(20, 80), 7, 6);
        assert_eq!(out.nrows(), (20 - 7) / 6 + 1);
        assert_eq!(out.ncols(), 80 * 7);
    }

    #[test]
    fn frames_are_stacked_in_order_with_the_given_stride() {
        let out = apply_lfr(&ramp(10, 2), 3, 2);
        // Output frame 1 starts at input frame 2 and stacks frames 2, 3, 4.
        assert_eq!(out.row(1).to_vec(), vec![200.0, 201.0, 300.0, 301.0, 400.0, 401.0]);
        assert_eq!(out.row(0).to_vec(), vec![0.0, 1.0, 100.0, 101.0, 200.0, 201.0]);
    }

    #[test]
    fn input_shorter_than_one_window_yields_no_frames() {
        let out = apply_lfr(&ramp(5, 4), 7, 6);
        assert_eq!(out.nrows(), 0);
        assert_eq!(out.ncols(), 4 * 7);
    }

    #[test]
    fn input_exactly_one_window_long_yields_one_frame() {
        assert_eq!(apply_lfr(&ramp(7, 3), 7, 6).nrows(), 1);
    }
}
