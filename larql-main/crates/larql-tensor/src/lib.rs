//! Tensor facade crate for LARQL.
//!
//! Provides a minimal abstraction over ndarray to enable future migration
//! to a custom tensor implementation without breaking the compute layer.
//!
//! The `ndarray` re-export (both as a submodule and flattened at crate root)
//! is the escape hatch for files that have not yet migrated to the idiomatic
//! [`DenseMatrix`] API. All consumer crates MUST go through this re-export —
//! they may not depend on `ndarray` directly.
//!
//! Consumer crates rename this crate to `ndarray` in their Cargo.toml via
//! `ndarray = { path = "../larql-tensor", package = "larql-tensor" }` so all
//! existing `use ndarray::X` paths continue to resolve.

#[cfg(feature = "blas")]
extern crate blas_src;

#[cfg(all(feature = "blas", target_os = "linux"))]
extern crate openblas_src;

pub use ::ndarray;
pub use ::ndarray::*;

pub mod sgemm;
pub use sgemm::{matmul_f32, matmul_transb_f32};

use thiserror::Error;

/// Errors from tensor operations.
#[derive(Debug, Error)]
pub enum TensorError {
    #[error("Shape mismatch: {0}")]
    ShapeMismatch(String),
    #[error("Matrix is not square: expected {0}x{0}, got {1}x{2}")]
    NotSquare(usize, usize, usize),
    #[error("Matrix is not positive definite: {0}")]
    NotPositiveDefinite(String),
    #[error("Linear solve failed: {0}")]
    SolveFailed(String),
}

/// Dense matrix wrapper around ndarray::Array2.
///
/// This facade provides a minimal abstraction that can be replaced
/// with a custom tensor implementation in future phases.
#[derive(Debug, Clone)]
pub struct DenseMatrix<T> {
    inner: Array2<T>,
}

impl<T> DenseMatrix<T>
where
    T: Clone + Copy,
{
    /// Create a matrix from raw data in row-major order.
    pub fn from_raw(data: Vec<T>, rows: usize, cols: usize) -> Result<Self, TensorError> {
        if data.len() != rows * cols {
            return Err(TensorError::ShapeMismatch(format!(
                "Data length {} does not match {}x{} shape",
                data.len(),
                rows,
                cols
            )));
        }
        let inner = Array2::from_shape_vec((rows, cols), data)
            .map_err(|e| TensorError::ShapeMismatch(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Get the number of rows.
    pub fn rows(&self) -> usize {
        self.inner.nrows()
    }

    /// Get the number of columns.
    pub fn cols(&self) -> usize {
        self.inner.ncols()
    }

    /// Get shape as (rows, cols).
    pub fn shape(&self) -> (usize, usize) {
        (self.inner.nrows(), self.inner.ncols())
    }

    /// Access the underlying ndarray as a view.
    pub fn as_array_view(&self) -> ndarray::ArrayView2<'_, T> {
        self.inner.view()
    }

    /// Access the underlying ndarray as a mutable view.
    pub fn as_array_view_mut(&mut self) -> ndarray::ArrayViewMut2<'_, T> {
        self.inner.view_mut()
    }

    /// Get a reference to the underlying ndarray.
    pub fn inner(&self) -> &Array2<T> {
        &self.inner
    }

    /// Get a mutable reference to the underlying ndarray.
    pub fn inner_mut(&mut self) -> &mut Array2<T> {
        &mut self.inner
    }
}

impl DenseMatrix<f64> {
    /// Matrix multiplication (dot product).
    pub fn dot(&self, other: &Self) -> Result<Self, TensorError> {
        if self.cols() != other.rows() {
            return Err(TensorError::ShapeMismatch(format!(
                "Cannot multiply {}x{} by {}x{}",
                self.rows(),
                self.cols(),
                other.rows(),
                other.cols()
            )));
        }
        let result = self.inner.dot(&other.inner);
        Ok(Self { inner: result })
    }

    /// Transpose the matrix.
    pub fn t(&self) -> Self {
        Self {
            inner: self.inner.t().to_owned(),
        }
    }

    /// Compute Cholesky decomposition.
    ///
    /// Returns L such that A = L * L^T where L is lower triangular.
    pub fn cholesky(&self) -> Result<Self, TensorError> {
        let (rows, cols) = self.shape();
        if rows != cols {
            return Err(TensorError::NotSquare(rows, rows, cols));
        }

        // Use ndarray-linalg's cholesky if available, otherwise implement manually
        // For now, we'll use a simple implementation
        let n = rows;
        let mut l = Array2::zeros((n, n));

        for i in 0..n {
            for j in 0..=i {
                let mut sum = 0.0;

                if j == i {
                    // Diagonal elements
                    for k in 0..j {
                        sum += l[[j, k]] * l[[j, k]];
                    }
                    let diag = self.inner[[j, j]] - sum;
                    if diag <= 0.0 {
                        return Err(TensorError::NotPositiveDefinite(format!(
                            "Matrix is not positive definite at index ({}, {})",
                            j, j
                        )));
                    }
                    l[[j, j]] = diag.sqrt();
                } else {
                    // Off-diagonal elements
                    for k in 0..j {
                        sum += l[[i, k]] * l[[j, k]];
                    }
                    l[[i, j]] = (self.inner[[i, j]] - sum) / l[[j, j]];
                }
            }
        }

        Ok(Self { inner: l })
    }

    /// Solve the linear system A * x = b using Cholesky decomposition.
    ///
    /// Returns x.
    pub fn solve(&self, b: &Self) -> Result<Self, TensorError> {
        let n = self.rows();
        if n != self.cols() {
            return Err(TensorError::NotSquare(n, n, self.cols()));
        }
        if b.rows() != n {
            return Err(TensorError::ShapeMismatch(format!(
                "b has {} rows, expected {}",
                b.rows(),
                n
            )));
        }

        let l = self.cholesky()?;
        let mut x = b.inner.clone();

        // Forward substitution: solve L * y = b
        for i in 0..n {
            for j in 0..i {
                x[[i, 0]] -= l.inner[[i, j]] * x[[j, 0]];
            }
            x[[i, 0]] /= l.inner[[i, i]];
        }

        // Backward substitution: solve L^T * x = y
        for i in (0..n).rev() {
            for j in (i + 1)..n {
                x[[i, 0]] -= l.inner[[j, i]] * x[[j, 0]];
            }
            x[[i, 0]] /= l.inner[[i, i]];
        }

        Ok(Self { inner: x })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_raw() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let m = DenseMatrix::from_raw(data, 2, 2).unwrap();
        assert_eq!(m.shape(), (2, 2));
        assert_eq!(m.rows(), 2);
        assert_eq!(m.cols(), 2);
    }

    #[test]
    fn test_from_raw_shape_mismatch() {
        let data = vec![1.0, 2.0, 3.0];
        assert!(DenseMatrix::from_raw(data, 2, 2).is_err());
    }

    #[test]
    fn test_dot() {
        let a = DenseMatrix::from_raw(vec![1.0, 2.0, 3.0, 4.0], 2, 2).unwrap();
        let b = DenseMatrix::from_raw(vec![5.0, 6.0, 7.0, 8.0], 2, 2).unwrap();
        let c = a.dot(&b).unwrap();
        assert_eq!(c.shape(), (2, 2));
        // [1*5 + 2*7, 1*6 + 2*8] = [19, 22]
        // [3*5 + 4*7, 3*6 + 4*8] = [43, 50]
        assert!((c.inner[[0, 0]] - 19.0).abs() < 1e-9);
        assert!((c.inner[[0, 1]] - 22.0).abs() < 1e-9);
        assert!((c.inner[[1, 0]] - 43.0).abs() < 1e-9);
        assert!((c.inner[[1, 1]] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_cholesky() {
        // Simple positive definite matrix
        let a = DenseMatrix::from_raw(
            vec![4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0],
            3,
            3,
        )
        .unwrap();
        let l = a.cholesky().unwrap();
        assert_eq!(l.shape(), (3, 3));

        // Verify L * L^T = A
        let ll_t = l.dot(&l.t()).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((ll_t.inner[[i, j]] - a.inner[[i, j]]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_cholesky_not_positive_definite() {
        // Not positive definite (has negative eigenvalue)
        let a = DenseMatrix::from_raw(vec![1.0, 2.0, 2.0, 1.0], 2, 2).unwrap();
        assert!(a.cholesky().is_err());
    }

    #[test]
    fn test_solve() {
        // Solve A * x = b where A is identity
        let a = DenseMatrix::from_raw(vec![1.0, 0.0, 0.0, 1.0], 2, 2).unwrap();
        let b = DenseMatrix::from_raw(vec![3.0, 4.0], 2, 1).unwrap();
        let x = a.solve(&b).unwrap();
        assert!((x.inner[[0, 0]] - 3.0).abs() < 1e-9);
        assert!((x.inner[[1, 0]] - 4.0).abs() < 1e-9);
    }
}
