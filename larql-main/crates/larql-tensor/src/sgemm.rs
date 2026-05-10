//! Pure-Rust sgemm via `matrixmultiply`.
//!
//! Replaces BLAS `cblas_sgemm` for the LARQL workloads. Used by
//! [`DenseMatrix::matmul`] and by `larql-compute`'s CPU matmul op.
//!
//! Phase 4 gate: within 2× of OpenBLAS/Accelerate on the three dominant
//! shapes captured in Phase 0 baselines. If perf gate fails, set the
//! `blas` feature on `larql-tensor` to route through ndarray+BLAS instead.

use ndarray::{Array2, ArrayView2};

/// C = A · B (row-major, contiguous).
pub fn matmul_f32(a: ArrayView2<f32>, b: ArrayView2<f32>) -> Array2<f32> {
    let (m, k) = (a.nrows(), a.ncols());
    let (kb, n) = (b.nrows(), b.ncols());
    assert_eq!(k, kb, "matmul shape mismatch: {}x{} * {}x{}", m, k, kb, n);

    let mut c = Array2::<f32>::zeros((m, n));

    // ndarray strides are in elements; matrixmultiply expects (row_stride, col_stride).
    let (arsc, acsc) = (a.strides()[0] as isize, a.strides()[1] as isize);
    let (brsc, bcsc) = (b.strides()[0] as isize, b.strides()[1] as isize);
    let (crsc, ccsc) = (c.strides()[0] as isize, c.strides()[1] as isize);

    unsafe {
        matrixmultiply::sgemm(
            m,
            k,
            n,
            1.0,
            a.as_ptr(),
            arsc,
            acsc,
            b.as_ptr(),
            brsc,
            bcsc,
            0.0,
            c.as_mut_ptr(),
            crsc,
            ccsc,
        );
    }

    c
}

/// C = A · B^T (row-major, contiguous). Equivalent to `matmul(a, b.t())`
/// but avoids materialising the transpose.
pub fn matmul_transb_f32(a: ArrayView2<f32>, b: ArrayView2<f32>) -> Array2<f32> {
    let (m, k) = (a.nrows(), a.ncols());
    let (n, kb) = (b.nrows(), b.ncols());
    assert_eq!(
        k, kb,
        "matmul_transb shape mismatch: {}x{} * ({}x{})^T",
        m, k, n, kb
    );

    let mut c = Array2::<f32>::zeros((m, n));

    let (arsc, acsc) = (a.strides()[0] as isize, a.strides()[1] as isize);
    // B is (n,k) treated as (k,n) via stride swap.
    let brsc_trans = b.strides()[1] as isize;
    let bcsc_trans = b.strides()[0] as isize;
    let (crsc, ccsc) = (c.strides()[0] as isize, c.strides()[1] as isize);

    unsafe {
        matrixmultiply::sgemm(
            m,
            k,
            n,
            1.0,
            a.as_ptr(),
            arsc,
            acsc,
            b.as_ptr(),
            brsc_trans,
            bcsc_trans,
            0.0,
            c.as_mut_ptr(),
            crsc,
            ccsc,
        );
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn matmul_identity() {
        let a = Array2::<f32>::from_shape_vec((2, 3), vec![1., 2., 3., 4., 5., 6.]).unwrap();
        let eye = Array2::<f32>::eye(3);
        let c = matmul_f32(a.view(), eye.view());
        assert_eq!(c, a);
    }

    #[test]
    fn matmul_known() {
        // A (2x3) * B (3x2)
        let a = Array2::from_shape_vec((2, 3), vec![1., 2., 3., 4., 5., 6.]).unwrap();
        let b = Array2::from_shape_vec((3, 2), vec![7., 8., 9., 10., 11., 12.]).unwrap();
        let c = matmul_f32(a.view(), b.view());
        // Row 0: [1*7+2*9+3*11, 1*8+2*10+3*12] = [58, 64]
        // Row 1: [4*7+5*9+6*11, 4*8+5*10+6*12] = [139, 154]
        assert_eq!(c.as_slice().unwrap(), &[58., 64., 139., 154.]);
    }

    #[test]
    fn matmul_transb_matches_manual_t() {
        let a = Array2::<f32>::from_shape_vec((2, 3), vec![1., 2., 3., 4., 5., 6.]).unwrap();
        let b = Array2::<f32>::from_shape_vec((4, 3), (0..12).map(|v| v as f32).collect()).unwrap();
        let got = matmul_transb_f32(a.view(), b.view());
        let expect = a.dot(&b.t());
        assert_eq!(got, expect);
    }
}
