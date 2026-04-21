//! Optimized mmap helpers for vindex file loading.
//!
//! Applies OS hints (madvise) to improve memory-mapped I/O performance:
//! - MADV_SEQUENTIAL: enables aggressive readahead for streaming access
//! - MADV_WILLNEED: prefaults pages into the page cache
//!
//! On M3 Max with 400 GB/s theoretical bandwidth, these hints can
//! improve effective throughput from ~50 GB/s to closer to peak.

use larql_core::mmap::Mmap;

/// Create an mmap with optimized access hints for streaming reads.
///
/// Safe to call on any file. The advisory hints are best-effort —
/// the OS may ignore them, but on macOS/Linux they significantly
/// improve page cache behavior for large sequential reads.
///
/// # Safety
///
/// The caller must ensure the file is not modified or truncated while the
/// mmap is alive.
pub unsafe fn mmap_optimized(file: &std::fs::File) -> Result<Mmap, std::io::Error> {
    let mmap = Mmap::map(file)?;
    mmap.advise_sequential();
    Ok(mmap)
}

/// Apply sequential + willneed hints to an existing mmap.
/// Call after Mmap::map() to optimize access patterns.
#[deprecated(note = "Use mmap.advise_sequential() directly")]
pub fn advise_sequential(mmap: &Mmap) {
    mmap.advise_sequential();
}
