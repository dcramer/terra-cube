//! Seeded randomness for the simulation. Everything random in a tick comes from one of these,
//! so a seed replays exactly: on a server, in a test, in a bug report.

/// SplitMix64: tiny, fast and good enough for behaviour.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E3779B97F4A7C15) ^ 0xD1B54A32D192ED03)
    }
    pub fn bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    /// 0 up to, not including, 1.
    pub fn unit(&mut self) -> f32 {
        (self.bits() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    /// A whole number from 0 up to, not including, `n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.bits() % n.max(1) as u64) as usize
    }
}
