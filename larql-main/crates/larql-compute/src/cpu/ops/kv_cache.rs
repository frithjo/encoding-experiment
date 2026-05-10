//! CPU KV cache management — dynamic sizing and cached attention.
//!
//! Per-layer flat vectors for cached K/V. Grows with generation.
//! At decode time: append new K/V, then attend Q against full cache.

/// A single layer's KV cache (CPU-side).
pub struct LayerKVCache {
    pub k_cache: Vec<f32>,
    pub v_cache: Vec<f32>,
    pub current_len: usize,
    pub max_seq: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
}

impl LayerKVCache {
    pub fn new(max_seq: usize, num_kv_heads: usize, head_dim: usize) -> Self {
        let size = max_seq * num_kv_heads * head_dim;
        Self {
            k_cache: vec![0.0; size],
            v_cache: vec![0.0; size],
            current_len: 0,
            max_seq,
            num_kv_heads,
            head_dim,
        }
    }

    /// Reset cache (for new prompt).
    pub fn clear(&mut self) {
        self.current_len = 0;
    }

    /// Grow cache buffers to a new maximum sequence length.
    pub fn grow(&mut self, new_max_seq: usize) {
        if new_max_seq <= self.max_seq {
            return;
        }
        let _old_size = self.max_seq * self.num_kv_heads * self.head_dim;
        let new_size = new_max_seq * self.num_kv_heads * self.head_dim;

        let mut new_k = vec![0.0; new_size];
        let mut new_v = vec![0.0; new_size];

        if self.current_len > 0 {
            let copy_len = self.current_len * self.num_kv_heads * self.head_dim;
            new_k[..copy_len].copy_from_slice(&self.k_cache[..copy_len]);
            new_v[..copy_len].copy_from_slice(&self.v_cache[..copy_len]);
        }

        self.k_cache = new_k;
        self.v_cache = new_v;
        self.max_seq = new_max_seq;
    }
}

/// Full KV cache for all layers (CPU-side).
pub struct KVCache {
    pub layers: Vec<LayerKVCache>,
}

impl KVCache {
    pub fn new(num_layers: usize, max_seq: usize, num_kv_heads: usize, head_dim: usize) -> Self {
        let layers = (0..num_layers)
            .map(|_| LayerKVCache::new(max_seq, num_kv_heads, head_dim))
            .collect();
        Self { layers }
    }

    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    pub fn grow(&mut self, new_max_seq: usize) {
        for layer in &mut self.layers {
            layer.grow(new_max_seq);
        }
    }

    pub fn current_len(&self) -> usize {
        self.layers.first().map(|l| l.current_len).unwrap_or(0)
    }
}
