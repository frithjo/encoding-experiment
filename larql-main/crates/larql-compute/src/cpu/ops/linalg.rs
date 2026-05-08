//! Linear algebra primitives for CPU — Norms, RoPE, and Cholesky.
//!
//! Basic operations (rms_norm, rope_at_pos) for forward pass.
//! Cholesky operations for MEMIT (experimental).

use ndarray::{Array1, Array2, ArrayView2, DenseMatrix};

/// Apply RMS Norm to a vector.
pub fn rms_norm(x: &[f32], weight: &[f32], eps: f32, offset: f32) -> Vec<f32> {
    let n = x.len();
    let mut ms = 0.0f32;
    for &v in x {
        ms += v * v;
    }
    ms /= n as f32;
    let inv_rms = 1.0 / (ms + eps).sqrt();

    let mut out = vec![0.0; n];
    for i in 0..n {
        out[i] = x[i] * inv_rms * (weight[i] + offset);
    }
    out
}

/// Apply RMS Norm to each row of a 2D array.
pub fn rms_norm_2d(x: ArrayView2<f32>, weight: &[f32], eps: f32, offset: f32) -> Array2<f32> {
    let (rows, cols) = x.dim();
    let mut out = Array2::zeros((rows, cols));
    for r in 0..rows {
        let row = x.row(r);
        let normed = rms_norm(row.as_slice().unwrap(), weight, eps, offset);
        out.row_mut(r).assign(&Array1::from(normed).view());
    }
    out
}

/// Apply Rotary Positional Embedding to a head vector at a specific position.
pub fn rope_at_pos(x: &mut [f32], head_dim: usize, base: f32, pos: usize) {
    let half_dim = head_dim / 2;
    for i in 0..half_dim {
        let theta = (pos as f32) / base.powf((2 * i) as f32 / head_dim as f32);
        let cos = theta.cos();
        let sin = theta.sin();

        let v0 = x[i];
        let v1 = x[i + half_dim];
        x[i] = v0 * cos - v1 * sin;
        x[i + half_dim] = v0 * sin + v1 * cos;
    }
}

/// Cholesky decomposition of a symmetric positive-definite matrix.
/// Returns the lower-triangular factor L such that A = L L^T.
///
/// Adds a small ridge to the diagonal before decomposition to
/// handle near-singular covariance matrices.
pub fn cholesky(a: &Array2<f64>, ridge: f64) -> Result<Array2<f64>, String> {
    let n = a.shape()[0];
    if a.shape()[1] != n {
        return Err(format!(
            "cholesky: matrix must be square, got {}×{}",
            n,
            a.shape()[1]
        ));
    }

    // Apply ridge to diagonal
    let mut a_with_ridge = a.clone();
    for i in 0..n {
        a_with_ridge[[i, i]] += ridge;
    }

    // Convert to DenseMatrix facade
    let data = a_with_ridge.as_slice().unwrap().to_vec();
    let dm = DenseMatrix::from_raw(data, n, n).map_err(|e| format!("cholesky: {}", e))?;

    // Use facade cholesky
    let l_dm = dm.cholesky().map_err(|e| format!("cholesky: {}", e))?;

    Ok(l_dm.inner().clone())
}

/// Solve L L^T X = B for X, given the lower-triangular Cholesky factor L.
/// B is (n, m) — solves m right-hand sides simultaneously.
pub fn cholesky_solve(l: &Array2<f64>, b: &Array2<f64>) -> Array2<f64> {
    let n = l.shape()[0];
    let m = b.shape()[1];

    // Forward substitution: L Y = B
    let mut y = Array2::<f64>::zeros((n, m));
    for i in 0..n {
        for col in 0..m {
            let mut sum = b[[i, col]];
            for k in 0..i {
                sum -= l[[i, k]] * y[[k, col]];
            }
            y[[i, col]] = sum / l[[i, i]];
        }
    }

    // Back substitution: L^T X = Y
    let mut x = Array2::<f64>::zeros((n, m));
    for i in (0..n).rev() {
        for col in 0..m {
            let mut sum = y[[i, col]];
            for k in (i + 1)..n {
                sum -= l[[k, i]] * x[[k, col]];
            }
            x[[i, col]] = sum / l[[i, i]];
        }
    }
    x
}

/// Compute A⁻¹ via Cholesky: solves L L^T X = I.
pub fn cholesky_inverse(l: &Array2<f64>) -> Array2<f64> {
    let n = l.shape()[0];
    let identity = Array2::<f64>::eye(n);
    cholesky_solve(l, &identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_cholesky_2x2() {
        // A = [[4, 2], [2, 3]] → L = [[2, 0], [1, √2]]
        let a = array![[4.0, 2.0], [2.0, 3.0]];
        let l = cholesky(&a, 0.0).unwrap();
        assert!((l[[0, 0]] - 2.0).abs() < 1e-10);
        assert!((l[[1, 0]] - 1.0).abs() < 1e-10);
        assert!((l[[1, 1]] - 2.0_f64.sqrt()).abs() < 1e-10);
        assert_eq!(l[[0, 1]], 0.0);
    }

    #[test]
    fn test_cholesky_solve_identity() {
        let a = Array2::<f64>::eye(3);
        let l = cholesky(&a, 0.0).unwrap();
        let b = array![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]];
        let x = cholesky_solve(&l, &b);
        for i in 0..3 {
            for j in 0..2 {
                assert!((x[[i, j]] - b[[i, j]]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_cholesky_inverse() {
        let a = array![[4.0, 2.0], [2.0, 3.0]];
        let l = cholesky(&a, 0.0).unwrap();
        let inv = cholesky_inverse(&l);
        // A * A⁻¹ should be I
        let product = a.dot(&inv);
        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (product[[i, j]] - expected).abs() < 1e-10,
                    "product[{i},{j}] = {} (expected {expected})",
                    product[[i, j]]
                );
            }
        }
    }

    #[test]
    fn test_cholesky_with_ridge() {
        // Negative diagonal fails; ridge rescues it.
        let mut a = Array2::<f64>::eye(3);
        a[[0, 0]] = -0.01;
        assert!(cholesky(&a, 0.0).is_err());
        let l = cholesky(&a, 0.1).unwrap();
        assert!(l[[0, 0]] > 0.0);
    }

    #[test]
    fn test_cholesky_not_positive_definite() {
        let a = array![[-1.0, 0.0], [0.0, 1.0]];
        assert!(cholesky(&a, 0.0).is_err());
    }
}
