//! Canonical MiniLM-L6-v2 INT8 encoder via `fastembed`.

use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use kite_mesh_proto::{CANONICAL_EMBEDDING_DIM, CANONICAL_MODEL_ID};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EncoderError {
    #[error("fastembed failed: {0}")]
    Fastembed(#[from] anyhow::Error),
    #[error("encoder returned {got} dims; expected {expected}")]
    Dimension { expected: u32, got: u32 },
    #[error("encoder returned empty batch")]
    Empty,
    #[error("encoder task panicked")]
    TaskJoin(#[from] tokio::task::JoinError),
    #[error("encoder mutex poisoned")]
    Poisoned,
}

/// Thread-safe handle to the canonical encoder.
#[derive(Clone)]
pub struct CanonicalEncoder {
    inner: Arc<Mutex<TextEmbedding>>,
}

impl CanonicalEncoder {
    /// Initialize the encoder — downloads MiniLM-L6-v2 quantized on first run.
    pub async fn new() -> Result<Self, EncoderError> {
        let model = tokio::task::spawn_blocking(|| {
            TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2Q))
                .map_err(EncoderError::Fastembed)
        })
        .await??;
        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
        })
    }

    pub fn model_id(&self) -> &'static str {
        CANONICAL_MODEL_ID
    }

    pub fn dimension(&self) -> u32 {
        CANONICAL_EMBEDDING_DIM
    }

    /// Encode a single text string. Returns an L2-normalized 384-dim vector.
    pub async fn encode(&self, text: &str) -> Result<Vec<f32>, EncoderError> {
        let inner = self.inner.clone();
        let owned = text.to_string();
        let vectors: Vec<Vec<f32>> = tokio::task::spawn_blocking(move || {
            let guard = inner.lock().map_err(|_| EncoderError::Poisoned)?;
            guard
                .embed(vec![owned], None)
                .map_err(EncoderError::Fastembed)
        })
        .await??;

        let mut vector = vectors.into_iter().next().ok_or(EncoderError::Empty)?;
        if vector.len() as u32 != CANONICAL_EMBEDDING_DIM {
            return Err(EncoderError::Dimension {
                expected: CANONICAL_EMBEDDING_DIM,
                got: vector.len() as u32,
            });
        }
        l2_normalize(&mut vector);
        Ok(vector)
    }
}

fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Cosine similarity on L2-normalized vectors reduces to a dot product.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "cosine: vector length mismatch");
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_identical_is_one() {
        let mut v = vec![0.0f32; CANONICAL_EMBEDDING_DIM as usize];
        v[0] = 1.0;
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }
}
