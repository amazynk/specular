//! Probability tables (a frame context), symbol counts and the backward adaptation process
//! (section 8.4).

use crate::tables::*;

pub(crate) type CoefProbs = [[[[[[u8; 3]; 6]; 6]; 2]; 2]; 4];

/// One set of probability tables (section 10.5 gives the defaults).
#[derive(Clone)]
pub(crate) struct Probs {
    /// tx_probs indexed by the largest allowed transform size (1 = 8x8, 2 = 16x16, 3 = 32x32).
    pub tx: [[[u8; 3]; 2]; 4],
    pub coef: CoefProbs,
    pub skip: [u8; 3],
    pub inter_mode: [[u8; 3]; 7],
    pub interp_filter: [[u8; 2]; 4],
    pub is_inter: [u8; 4],
    pub comp_mode: [u8; 5],
    pub single_ref: [[u8; 2]; 5],
    pub comp_ref: [u8; 5],
    pub y_mode: [[u8; 9]; 4],
    pub uv_mode: [[u8; 9]; 10],
    pub partition: [[u8; 3]; 16],
    pub mv_joint: [u8; 3],
    pub mv_sign: [u8; 2],
    pub mv_class: [[u8; 10]; 2],
    pub mv_class0_bit: [u8; 2],
    pub mv_bits: [[u8; 10]; 2],
    pub mv_class0_fr: [[[u8; 3]; 2]; 2],
    pub mv_fr: [[u8; 3]; 2],
    pub mv_class0_hp: [u8; 2],
    pub mv_hp: [u8; 2],
}

impl Default for Probs {
    fn default() -> Self {
        Probs {
            tx: DEFAULT_TX_PROBS,
            coef: DEFAULT_COEF_PROBS,
            skip: DEFAULT_SKIP_PROB,
            inter_mode: DEFAULT_INTER_MODE_PROBS,
            interp_filter: DEFAULT_INTERP_FILTER_PROBS,
            is_inter: DEFAULT_IS_INTER_PROB,
            comp_mode: DEFAULT_COMP_MODE_PROB,
            single_ref: DEFAULT_SINGLE_REF_PROB,
            comp_ref: DEFAULT_COMP_REF_PROB,
            y_mode: DEFAULT_Y_MODE_PROBS,
            uv_mode: DEFAULT_UV_MODE_PROBS,
            partition: DEFAULT_PARTITION_PROBS,
            mv_joint: DEFAULT_MV_JOINT_PROBS,
            mv_sign: DEFAULT_MV_SIGN_PROB,
            mv_class: DEFAULT_MV_CLASS_PROBS,
            mv_class0_bit: DEFAULT_MV_CLASS0_BIT_PROB,
            mv_bits: DEFAULT_MV_BITS_PROB,
            mv_class0_fr: DEFAULT_MV_CLASS0_FR_PROBS,
            mv_fr: DEFAULT_MV_FR_PROBS,
            mv_class0_hp: DEFAULT_MV_CLASS0_HP_PROB,
            mv_hp: DEFAULT_MV_HP_PROB,
        }
    }
}

impl Probs {
    /// load_probs( ctx ): everything except tx_probs and skip_prob.
    pub(crate) fn load_probs(&mut self, saved: &Probs) {
        let tx = self.tx;
        let skip = self.skip;
        *self = saved.clone();
        self.tx = tx;
        self.skip = skip;
    }

    /// load_probs2( ctx ): tx_probs and skip_prob.
    pub(crate) fn load_probs2(&mut self, saved: &Probs) {
        self.tx = saved.tx;
        self.skip = saved.skip;
    }
}

/// Symbol counts of one frame (section 8.3).
#[derive(Clone, Default)]
pub(crate) struct Counts {
    pub intra_mode: [[u32; 10]; 4],
    pub uv_mode: [[u32; 10]; 10],
    pub partition: [[u32; 4]; 16],
    pub interp_filter: [[u32; 3]; 4],
    pub inter_mode: [[u32; 4]; 7],
    pub tx: [[[u32; 4]; 2]; 4],
    pub is_inter: [[u32; 2]; 4],
    pub comp_mode: [[u32; 2]; 5],
    pub single_ref: [[[u32; 2]; 2]; 5],
    pub comp_ref: [[u32; 2]; 5],
    pub skip: [[u32; 2]; 3],
    pub mv_joint: [u32; 4],
    pub mv_sign: [[u32; 2]; 2],
    pub mv_class: [[u32; 11]; 2],
    pub mv_class0_bit: [[u32; 2]; 2],
    pub mv_class0_fr: [[[u32; 4]; 2]; 2],
    pub mv_class0_hp: [[u32; 2]; 2],
    pub mv_bits: [[[u32; 2]; 10]; 2],
    pub mv_fr: [[u32; 4]; 2],
    pub mv_hp: [[u32; 2]; 2],
    pub token: [[[[[[u32; 3]; 6]; 6]; 2]; 2]; 4],
    pub more_coefs: [[[[[[u32; 2]; 6]; 6]; 2]; 2]; 4],
}

fn add<const N: usize>(a: &mut [u32; N], b: &[u32; N]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x = x.wrapping_add(*y);
    }
}

impl Counts {
    /// Adds the counts of another tile.
    pub(crate) fn merge(&mut self, o: &Counts) {
        for i in 0..4 {
            add(&mut self.intra_mode[i], &o.intra_mode[i]);
            add(&mut self.interp_filter[i], &o.interp_filter[i]);
            add(&mut self.is_inter[i], &o.is_inter[i]);
            for j in 0..2 {
                add(&mut self.tx[i][j], &o.tx[i][j]);
            }
        }
        for i in 0..10 {
            add(&mut self.uv_mode[i], &o.uv_mode[i]);
        }
        for i in 0..16 {
            add(&mut self.partition[i], &o.partition[i]);
        }
        for i in 0..7 {
            add(&mut self.inter_mode[i], &o.inter_mode[i]);
        }
        for i in 0..5 {
            add(&mut self.comp_mode[i], &o.comp_mode[i]);
            add(&mut self.comp_ref[i], &o.comp_ref[i]);
            add(&mut self.single_ref[i][0], &o.single_ref[i][0]);
            add(&mut self.single_ref[i][1], &o.single_ref[i][1]);
        }
        for i in 0..3 {
            add(&mut self.skip[i], &o.skip[i]);
        }
        add(&mut self.mv_joint, &o.mv_joint);
        for i in 0..2 {
            add(&mut self.mv_sign[i], &o.mv_sign[i]);
            add(&mut self.mv_class[i], &o.mv_class[i]);
            add(&mut self.mv_class0_bit[i], &o.mv_class0_bit[i]);
            add(&mut self.mv_class0_fr[i][0], &o.mv_class0_fr[i][0]);
            add(&mut self.mv_class0_fr[i][1], &o.mv_class0_fr[i][1]);
            add(&mut self.mv_class0_hp[i], &o.mv_class0_hp[i]);
            for j in 0..10 {
                add(&mut self.mv_bits[i][j], &o.mv_bits[i][j]);
            }
            add(&mut self.mv_fr[i], &o.mv_fr[i]);
            add(&mut self.mv_hp[i], &o.mv_hp[i]);
        }
        for t in 0..4 {
            for i in 0..2 {
                for j in 0..2 {
                    for k in 0..6 {
                        for l in 0..6 {
                            add(&mut self.token[t][i][j][k][l], &o.token[t][i][j][k][l]);
                            add(
                                &mut self.more_coefs[t][i][j][k][l],
                                &o.more_coefs[t][i][j][k][l],
                            );
                        }
                    }
                }
            }
        }
    }
}

/// merge_prob (8.4.1).
fn merge_prob(pre: u8, ct0: u32, ct1: u32, count_sat: u32, max_update: u32) -> u8 {
    let den = ct0 as u64 + ct1 as u64;
    let prob = (ct0 as u64 * 256 + (den >> 1))
        .checked_div(den)
        .map_or(128, |p| p.clamp(1, 255) as u32);
    let count = den.min(count_sat as u64) as u32;
    let factor = max_update * count / count_sat;
    ((pre as u32 * (256 - factor) + prob * factor + 128) >> 8) as u8
}

/// merge_probs (8.4.2): adapts the probabilities of a tree from its leaf counts.
fn merge_probs(
    tree: &[i8],
    i: usize,
    probs: &mut [u8],
    counts: &[u32],
    count_sat: u32,
    max_update: u32,
) -> u32 {
    let s = tree[i];
    let left = if s <= 0 {
        counts[(-s) as usize]
    } else {
        merge_probs(tree, s as usize, probs, counts, count_sat, max_update)
    };
    let r = tree[i + 1];
    let right = if r <= 0 {
        counts[(-r) as usize]
    } else {
        merge_probs(tree, r as usize, probs, counts, count_sat, max_update)
    };
    probs[i >> 1] = merge_prob(probs[i >> 1], left, right, count_sat, max_update);
    left.wrapping_add(right)
}

fn adapt_probs(tree: &[i8], probs: &mut [u8], counts: &[u32]) {
    merge_probs(tree, 0, probs, counts, 20, 128);
}

fn adapt_prob(p: &mut u8, c: &[u32; 2]) {
    *p = merge_prob(*p, c[0], c[1], 20, 128);
}

/// adapt_coef_probs (8.4.3). `fc` holds the probabilities loaded from the saved context.
pub(crate) fn adapt_coef_probs(fc: &mut Probs, counts: &Counts, intra: bool, last_was_key: bool) {
    let update = if intra {
        112
    } else if last_was_key {
        128
    } else {
        112
    };
    for t in 0..4 {
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..6 {
                    let max_l = if k == 0 { 3 } else { 6 };
                    for l in 0..max_l {
                        let p = &mut fc.coef[t][i][j][k][l];
                        merge_probs(
                            &SMALL_TOKEN_TREE,
                            2,
                            p,
                            &counts.token[t][i][j][k][l],
                            24,
                            update,
                        );
                        merge_probs(
                            &BINARY_TREE,
                            0,
                            p,
                            &counts.more_coefs[t][i][j][k][l],
                            24,
                            update,
                        );
                    }
                }
            }
        }
    }
}

/// adapt_noncoef_probs (8.4.4).
pub(crate) fn adapt_noncoef_probs(
    fc: &mut Probs,
    c: &Counts,
    interp_switchable: bool,
    tx_select: bool,
    allow_hp: bool,
) {
    for i in 0..4 {
        adapt_prob(&mut fc.is_inter[i], &c.is_inter[i]);
    }
    for i in 0..5 {
        adapt_prob(&mut fc.comp_mode[i], &c.comp_mode[i]);
    }
    for i in 0..5 {
        adapt_prob(&mut fc.comp_ref[i], &c.comp_ref[i]);
    }
    for i in 0..5 {
        for j in 0..2 {
            adapt_prob(&mut fc.single_ref[i][j], &c.single_ref[i][j]);
        }
    }
    for i in 0..7 {
        adapt_probs(&INTER_MODE_TREE, &mut fc.inter_mode[i], &c.inter_mode[i]);
    }
    for i in 0..4 {
        adapt_probs(&INTRA_MODE_TREE, &mut fc.y_mode[i], &c.intra_mode[i]);
    }
    for i in 0..10 {
        adapt_probs(&INTRA_MODE_TREE, &mut fc.uv_mode[i], &c.uv_mode[i]);
    }
    for i in 0..16 {
        adapt_probs(&PARTITION_TREE, &mut fc.partition[i], &c.partition[i]);
    }
    for i in 0..3 {
        adapt_prob(&mut fc.skip[i], &c.skip[i]);
    }
    if interp_switchable {
        for i in 0..4 {
            adapt_probs(
                &INTERP_FILTER_TREE,
                &mut fc.interp_filter[i],
                &c.interp_filter[i],
            );
        }
    }
    if tx_select {
        for i in 0..2 {
            adapt_probs(&TX_SIZE_8_TREE, &mut fc.tx[1][i], &c.tx[1][i]);
            adapt_probs(&TX_SIZE_16_TREE, &mut fc.tx[2][i], &c.tx[2][i]);
            adapt_probs(&TX_SIZE_32_TREE, &mut fc.tx[3][i], &c.tx[3][i]);
        }
    }
    adapt_probs(&MV_JOINT_TREE, &mut fc.mv_joint, &c.mv_joint);
    for i in 0..2 {
        adapt_prob(&mut fc.mv_sign[i], &c.mv_sign[i]);
        adapt_probs(&MV_CLASS_TREE, &mut fc.mv_class[i], &c.mv_class[i]);
        adapt_prob(&mut fc.mv_class0_bit[i], &c.mv_class0_bit[i]);
        for j in 0..10 {
            adapt_prob(&mut fc.mv_bits[i][j], &c.mv_bits[i][j]);
        }
        for j in 0..2 {
            adapt_probs(
                &MV_FR_TREE,
                &mut fc.mv_class0_fr[i][j],
                &c.mv_class0_fr[i][j],
            );
        }
        adapt_probs(&MV_FR_TREE, &mut fc.mv_fr[i], &c.mv_fr[i]);
        if allow_hp {
            adapt_prob(&mut fc.mv_class0_hp[i], &c.mv_class0_hp[i]);
            adapt_prob(&mut fc.mv_hp[i], &c.mv_hp[i]);
        }
    }
}

/// The token probabilities of every model probability (section 9.3.2, `pareto`), indexed by
/// coef_probs[..][2] and the tree node minus 2.
pub(crate) static PARETO_FULL: std::sync::LazyLock<[[u8; 8]; 256]> =
    std::sync::LazyLock::new(|| {
        let mut t = [[0u8; 8]; 256];
        for (p, row) in t.iter_mut().enumerate() {
            let p = p.max(1);
            let x = (p - 1) / 2;
            for (k, v) in row.iter_mut().enumerate() {
                *v = if p & 1 == 1 {
                    PARETO_TABLE[x][k]
                } else {
                    ((PARETO_TABLE[x][k] as u32 + PARETO_TABLE[x + 1][k] as u32) >> 1) as u8
                };
            }
        }
        t
    });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prob_saturates() {
        // No counts: unchanged.
        assert_eq!(merge_prob(100, 0, 0, 20, 128), 100);
        // Many zeros: moves halfway to 255.
        assert_eq!(
            merge_prob(1, 1000, 0, 20, 128) as u32,
            (128 + 255 * 128 + 128) >> 8
        );
    }

    #[test]
    fn tree_shapes() {
        // Every tree has one more leaf than its probabilities.
        for (t, n) in [
            (&INTRA_MODE_TREE[..], 10),
            (&PARTITION_TREE[..], 4),
            (&MV_CLASS_TREE[..], 11),
            (&TOKEN_TREE[..], 11),
            (&SEGMENT_TREE[..], 8),
        ] {
            let leaves = t.iter().filter(|&&v| v <= 0).count();
            assert_eq!(leaves, n);
            assert_eq!(t.len(), 2 * (n - 1));
        }
    }
}
