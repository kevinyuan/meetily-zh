use ndarray::ArrayView3;

/// Result of CTC greedy decoding for a single utterance.
pub struct CtcDecoderResult {
    /// Decoded token IDs (excluding blanks and collapsed repeats).
    pub tokens: Vec<i64>,
    /// Frame indices corresponding to each decoded token.
    pub timestamps: Vec<i32>,
}

/// CTC greedy search decoder.
///
/// For each time step, selects the token with highest logit. Skips blank tokens
/// and consecutive repeated tokens.
pub fn ctc_greedy_decode(
    logits: &ArrayView3<f32>,
    logits_lengths: &[i64],
    blank_id: i64,
) -> Vec<CtcDecoderResult> {
    let batch_size = logits.shape()[0];
    let vocab_size = logits.shape()[2];
    let mut results = Vec::with_capacity(batch_size);

    for b in 0..batch_size {
        let num_frames = logits_lengths[b] as usize;
        let mut result = CtcDecoderResult {
            tokens: Vec::new(),
            timestamps: Vec::new(),
        };
        let mut prev_id: i64 = -1;

        for t in 0..num_frames {
            let mut max_val = f32::NEG_INFINITY;
            let mut max_id: i64 = 0;
            for v in 0..vocab_size {
                let val = logits[[b, t, v]];
                if val > max_val {
                    max_val = val;
                    max_id = v as i64;
                }
            }

            if max_id != blank_id && max_id != prev_id {
                result.tokens.push(max_id);
                result.timestamps.push(t as i32);
            }
            prev_id = max_id;
        }

        results.push(result);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array3;

    /// Build logits [1, frames, vocab] whose argmax at each frame is `path[t]`.
    fn logits_for(path: &[usize], vocab: usize) -> Array3<f32> {
        let mut logits = Array3::zeros((1, path.len(), vocab));
        for (t, &id) in path.iter().enumerate() {
            logits[[0, t, id]] = 10.0;
        }
        logits
    }

    fn decode(path: &[usize], blank: i64) -> CtcDecoderResult {
        let logits = logits_for(path, 6);
        ctc_greedy_decode(&logits.view(), &[path.len() as i64], blank)
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn blanks_are_dropped() {
        let result = decode(&[0, 3, 0, 4, 0], 0);
        assert_eq!(result.tokens, vec![3, 4]);
    }

    #[test]
    fn consecutive_repeats_collapse_to_one_token() {
        let result = decode(&[2, 2, 2, 5, 5], 0);
        assert_eq!(result.tokens, vec![2, 5]);
    }

    #[test]
    fn a_blank_between_repeats_keeps_both() {
        // "AA" is only representable by separating the two with a blank.
        let result = decode(&[2, 0, 2], 0);
        assert_eq!(result.tokens, vec![2, 2]);
    }

    #[test]
    fn timestamps_are_the_frame_where_each_token_first_appears() {
        let result = decode(&[0, 0, 3, 3, 0, 4], 0);
        assert_eq!(result.tokens, vec![3, 4]);
        assert_eq!(result.timestamps, vec![2, 5]);
    }

    #[test]
    fn only_the_valid_length_is_decoded() {
        let logits = logits_for(&[3, 4, 5], 6);
        let result = &ctc_greedy_decode(&logits.view(), &[2], 0)[0];
        assert_eq!(result.tokens, vec![3, 4], "the padded third frame must be ignored");
    }

    #[test]
    fn all_blank_input_decodes_to_nothing() {
        let result = decode(&[0, 0, 0], 0);
        assert!(result.tokens.is_empty() && result.timestamps.is_empty());
    }

    #[test]
    fn blank_need_not_be_id_zero() {
        let result = decode(&[5, 1, 5, 2], 5);
        assert_eq!(result.tokens, vec![1, 2]);
    }
}
