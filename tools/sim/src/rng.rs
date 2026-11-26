//! A small deterministic generator (splitmix64): one seed, one sequence, on every host.

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// The seed of walk `index` of a run started with `seed`.
    pub fn walk_seed(seed: u64, index: u64) -> u64 {
        let mut mix = Self(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ index);
        mix.next_u64()
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// An index drawn by weight; `None` when every weight is zero.
    pub fn weighted(&mut self, weights: &[f64]) -> Option<usize> {
        let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }
        let mut target = self.unit() * total;
        for (index, weight) in weights.iter().enumerate() {
            if *weight <= 0.0 {
                continue;
            }
            if target < *weight {
                return Some(index);
            }
            target -= weight;
        }
        weights.iter().rposition(|w| *w > 0.0)
    }
}
