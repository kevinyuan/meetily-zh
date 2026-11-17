use ndarray::{Array1, Array2};

/// Apply Cepstral Mean-Variance Normalization (CMVN).
///
/// Formula: `x[i] = (x[i] + neg_mean[i]) * inv_stddev[i]`
///
/// Modifies features in-place.
pub fn apply_cmvn(features: &mut Array2<f32>, neg_mean: &Array1<f32>, inv_stddev: &Array1<f32>) {
    let dim = features.ncols();
    debug_assert_eq!(neg_mean.len(), dim);
    debug_assert_eq!(inv_stddev.len(), dim);

    for mut row in features.rows_mut() {
        for j in 0..dim {
            row[j] = (row[j] + neg_mean[j]) * inv_stddev[j];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{arr1, arr2};

    #[test]
    fn applies_add_mean_then_scale_per_column() {
        let mut features = arr2(&[[1.0_f32, 2.0], [3.0, 4.0]]);
        // x = (x + neg_mean) * inv_stddev
        apply_cmvn(&mut features, &arr1(&[-1.0, -2.0]), &arr1(&[2.0, 0.5]));
        assert_eq!(features, arr2(&[[0.0, 0.0], [4.0, 1.0]]));
    }

    #[test]
    fn identity_parameters_leave_features_unchanged() {
        let original = arr2(&[[0.5_f32, -1.5, 2.0]]);
        let mut features = original.clone();
        apply_cmvn(&mut features, &arr1(&[0.0, 0.0, 0.0]), &arr1(&[1.0, 1.0, 1.0]));
        assert_eq!(features, original);
    }
}
