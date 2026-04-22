//! Memory-mapped file wrapper using libc::mmap directly.
//!
//! Replaces memmap2 dependency with a minimal in-tree implementation.
//! Platform-specific: Unix-only via libc::mmap/munmap/madvise.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::ptr;
use std::slice;

/// Memory-mapped file view.
///
/// # Safety
///
/// The caller must ensure the file is not modified or truncated while the
/// Mmap is alive. This is the standard mmap safety contract.
pub struct Mmap {
    ptr: *const u8,
    len: usize,
}

unsafe impl Send for Mmap {}
unsafe impl Sync for Mmap {}

impl Mmap {
    /// Create a read-only memory mapping of a file.
    ///
    /// # Safety
    ///
    /// The caller must ensure the file is not modified or truncated while the
    /// Mmap is alive.
    pub unsafe fn map(file: &File) -> io::Result<Mmap> {
        let metadata = file.metadata()?;
        let len = metadata.len() as usize;

        if len == 0 {
            return Ok(Mmap {
                ptr: ptr::null(),
                len: 0,
            });
        }

        #[cfg(unix)]
        {
            use libc::{MAP_PRIVATE, PROT_READ};
            let fd = file.as_raw_fd();

            let ptr = libc::mmap(ptr::null_mut(), len, PROT_READ, MAP_PRIVATE, fd, 0);

            if ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }

            Ok(Mmap {
                ptr: ptr as *const u8,
                len,
            })
        }

        #[cfg(not(unix))]
        {
            // Windows support not implemented; keep memmap2 for Windows builds
            compile_error!("Windows support requires CreateFileMappingW/MapViewOfFile");
        }
    }

    /// Apply sequential + willneed hints to optimize streaming access.
    ///
    /// Best-effort advisory — the OS may ignore these hints.
    pub fn advise_sequential(&self) {
        if self.len == 0 {
            return;
        }

        #[cfg(unix)]
        {
            unsafe {
                libc::madvise(self.ptr as *mut c_void, self.len, libc::MADV_SEQUENTIAL);
                libc::madvise(self.ptr as *mut c_void, self.len, libc::MADV_WILLNEED);
            }
        }
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        if self.len > 0 && !self.ptr.is_null() {
            #[cfg(unix)]
            unsafe {
                libc::munmap(self.ptr as *mut c_void, self.len);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_mmap_basic() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();

        let mmap = unsafe { Mmap::map(file.as_file()).unwrap() };
        assert_eq!(&*mmap, b"hello world");
    }

    #[test]
    fn test_mmap_empty() {
        let file = NamedTempFile::new().unwrap();
        let mmap = unsafe { Mmap::map(file.as_file()).unwrap() };
        assert_eq!(mmap.len(), 0);
        assert!(mmap.ptr.is_null());
    }

    #[test]
    fn test_mmap_advise() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&vec![0u8; 4096]).unwrap();

        let mmap = unsafe { Mmap::map(file.as_file()).unwrap() };
        mmap.advise_sequential(); // Should not panic
        assert_eq!(mmap.len(), 4096);
    }
}
