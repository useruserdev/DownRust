// SPDX-License-Identifier: GPL-3.0-or-later
//! File handling: part files, positioned writes and the final rename.
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::util::filename::unique_file_name;

const RENAME_RETRIES: u32 = 10;
const RENAME_DELAY: Duration = Duration::from_millis(200);

/// Opens the part file for reading and writing, creating it when missing.
pub(crate) fn open_part_file(path: &Path, truncate: bool) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(truncate)
        .open(path)
}

/// Reserves `size` bytes for a fresh part file. On Windows the file is first marked sparse, so the
/// reserved range is not zero-filled on disk. Returns true when the file was marked sparse.
pub(crate) fn preallocate(file: &File, size: u64) -> io::Result<bool> {
    let sparse = mark_sparse(file);
    file.set_len(size)?;
    Ok(sparse)
}

/// Asks NTFS to store the file as sparse. Failure (for example on FAT) is not an error.
#[cfg(windows)]
fn mark_sparse(file: &File) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    let mut returned: u32 = 0;
    // SAFETY: the handle comes from `file`, which stays open during the call. FSCTL_SET_SPARSE
    // takes no input or output buffer, so the null pointers and zero sizes are valid.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    ok != 0
}

#[cfg(not(windows))]
fn mark_sparse(_file: &File) -> bool {
    false
}

/// Writes the whole buffer at the given offset without moving a shared cursor.
#[cfg(windows)]
pub(crate) fn write_all_at(file: &File, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_write(buf, offset) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(n) => {
                buf = &buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Writes the whole buffer at the given offset without moving a shared cursor.
#[cfg(unix)]
pub(crate) fn write_all_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
}

/// Removes a file, treating a missing file as success.
pub(crate) fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Renames the finished part file to a free name in `dir`. Returns the name used.
pub(crate) async fn finalize(part: &Path, dir: &Path, desired: &str) -> io::Result<String> {
    let name = unique_file_name(desired, |n| dir.join(n).exists());
    let target = dir.join(&name);
    let mut retries = 0;
    loop {
        match std::fs::rename(part, &target) {
            Ok(()) => return Ok(name),
            Err(_) if retries < RENAME_RETRIES => {
                retries += 1;
                tokio::time::sleep(RENAME_DELAY).await;
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finalize_uses_unique_name_when_taken() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"old").unwrap();
        let part = dir.path().join("a.bin.downrust");
        std::fs::write(&part, b"new").unwrap();
        let name = finalize(&part, dir.path(), "a.bin").await.unwrap();
        assert_eq!(name, "a (1).bin");
        assert_eq!(std::fs::read(dir.path().join("a (1).bin")).unwrap(), b"new");
        assert!(!part.exists());
    }

    #[test]
    fn preallocate_sets_the_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pre.part");
        let file = open_part_file(&path, true).unwrap();
        preallocate(&file, 4096).unwrap();
        drop(file);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 4096);
    }

    #[cfg(windows)]
    #[test]
    fn preallocated_part_file_is_sparse_when_the_volume_supports_it() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileAttributesW, FILE_ATTRIBUTE_SPARSE_FILE,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sparse.part");
        let file = open_part_file(&path, true).unwrap();
        let sparse = preallocate(&file, 1 << 20).unwrap();
        drop(file);
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a valid, NUL-terminated UTF-16 string that outlives the call.
        let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 1 << 20);
        if sparse {
            assert_ne!(attributes & FILE_ATTRIBUTE_SPARSE_FILE, 0);
        }
    }

    #[test]
    fn remove_if_exists_ignores_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(remove_if_exists(&dir.path().join("none")).is_ok());
    }
}
