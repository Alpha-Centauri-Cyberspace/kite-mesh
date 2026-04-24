//! Angular (random-hyperplane) LSH.
//!
//! `L` independent tables, each producing a `k`-bit signature. Hyperplanes
//! are seeded deterministically from `(model_id, epoch)` so two peers with
//! the same encoder produce identical signatures.

use blake3::Hasher;

/// Walking-skeleton defaults. PRD §7 suggests starter ranges `k = 8..12`,
/// `L = 16..32`. The skeleton uses the low end for speed.
pub const DEFAULT_K: u32 = 8;
pub const DEFAULT_L: u32 = 4;

#[derive(Debug, Clone, Copy)]
pub struct LshParams {
    pub k: u32,
    pub l: u32,
    pub dim: u32,
}

impl LshParams {
    pub fn default_for_skeleton() -> Self {
        Self {
            k: DEFAULT_K,
            l: DEFAULT_L,
            dim: kite_mesh_proto::CANONICAL_EMBEDDING_DIM,
        }
    }
}

/// Angular LSH parameterized by a seed derived from `(model_id, epoch)`.
pub struct AngularLsh {
    params: LshParams,
    planes: Vec<Vec<f32>>, // length = l * k; each plane has `dim` floats
}

impl AngularLsh {
    pub fn new(params: LshParams, seed: &[u8]) -> Self {
        let mut planes = Vec::with_capacity((params.l * params.k) as usize);
        for table in 0..params.l {
            for bit in 0..params.k {
                planes.push(generate_plane(seed, table, bit, params.dim));
            }
        }
        Self { params, planes }
    }

    pub fn params(&self) -> LshParams {
        self.params
    }

    /// Produce `L` signatures, one per table. Each signature is `ceil(k / 8)`
    /// bytes with the sign bit of `v · plane_i` packed into each bit.
    pub fn signatures(&self, v: &[f32]) -> Vec<Vec<u8>> {
        assert_eq!(v.len() as u32, self.params.dim);
        let bytes_per = self.params.k.div_ceil(8) as usize;
        let mut out = Vec::with_capacity(self.params.l as usize);
        for table in 0..self.params.l {
            let mut sig = vec![0u8; bytes_per];
            for bit in 0..self.params.k {
                let idx = (table * self.params.k + bit) as usize;
                let plane = &self.planes[idx];
                let dot = v.iter().zip(plane).map(|(x, y)| x * y).sum::<f32>();
                if dot >= 0.0 {
                    let byte = (bit / 8) as usize;
                    let bit_in_byte = (bit % 8) as u8;
                    sig[byte] |= 1 << bit_in_byte;
                }
            }
            out.push(sig);
        }
        out
    }
}

/// Deterministically generate a plane from the seed. blake3 is expanded into
/// enough bytes for `dim` pseudo-Gaussian floats via Box–Muller on u32 halves.
fn generate_plane(seed: &[u8], table: u32, bit: u32, dim: u32) -> Vec<f32> {
    let mut hasher = Hasher::new();
    hasher.update(seed);
    hasher.update(&table.to_le_bytes());
    hasher.update(&bit.to_le_bytes());
    let mut xof = hasher.finalize_xof();

    let mut bytes = vec![0u8; (dim as usize) * 8];
    xof.fill(&mut bytes);

    let mut plane = Vec::with_capacity(dim as usize);
    let mut i = 0;
    while plane.len() < dim as usize {
        let u1 = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let u2 = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap());
        i += 8;
        // Avoid u1 == 0 (ln(0) undefined).
        let r1 = ((u1 as f64) + 1.0) / (u32::MAX as f64 + 2.0);
        let r2 = (u2 as f64) / (u32::MAX as f64 + 1.0);
        let z = (-2.0 * r1.ln()).sqrt() * (2.0 * std::f64::consts::PI * r2).cos();
        plane.push(z as f32);
    }
    plane
}

/// Seed for the canonical encoder's LSH planes in a given epoch.
pub fn default_seed() -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(kite_mesh_proto::CANONICAL_MODEL_ID.as_bytes());
    h.update(&kite_mesh_proto::PROTOCOL_VERSION.to_le_bytes());
    h.finalize().as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(dim: u32, axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim as usize];
        v[axis] = 1.0;
        v
    }

    #[test]
    fn deterministic_signatures_across_instances() {
        let params = LshParams::default_for_skeleton();
        let seed = default_seed();
        let a = AngularLsh::new(params, &seed);
        let b = AngularLsh::new(params, &seed);
        let v = unit(params.dim, 0);
        assert_eq!(a.signatures(&v), b.signatures(&v));
    }

    #[test]
    fn different_vectors_differ() {
        let params = LshParams::default_for_skeleton();
        let lsh = AngularLsh::new(params, &default_seed());
        let v1 = unit(params.dim, 0);
        let v2 = unit(params.dim, 1);
        // Not strictly required (could collide), but overwhelmingly likely.
        assert_ne!(lsh.signatures(&v1), lsh.signatures(&v2));
    }
}
