use std::{
    cmp, fs, io, mem,
    os::windows::fs::MetadataExt,
    os::windows::io::{AsRawHandle, RawHandle},
    path::Path,
    ptr,
};

use winapi::um::{
    fileapi::GetVolumeInformationByHandleW,
    ioapiset::DeviceIoControl,
    winioctl::{
        FSCTL_GET_INTEGRITY_INFORMATION, FSCTL_SET_INTEGRITY_INFORMATION, FSCTL_SET_SPARSE,
    },
    {FILE_ATTRIBUTE_SPARSE_FILE, FILE_SUPPORTS_BLOCK_REFCOUNTING},
};

use super::utility::NamedTempFile;

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    // Inspired by https://github.com/0xbadfca11/reflink/blob/master/reflink.cpp
    let src = fs::File::open(&from)?;

    let src_metadata = src.metadata()?;
    let src_file_size = src_metadata.file_size();
    let src_is_sparse = src_metadata.file_attributes() & FILE_ATTRIBUTE_SPARSE_FILE > 0;

    let dest = NamedTempFile::create_new(to)?;

    if src_is_sparse {
        dest.set_sparse()?;
    }

    let src_integrity_info = src.get_integrity_information()?;
    let cluster_size = src_integrity_info.ClusterSizeInBytes as i64;
    if cluster_size != 0 {
        // Cluster size must either be 4K or 64K (restricted by ReFS)
        assert!(cluster_size == 4 * 1024 || cluster_size == 64 * 1024);
        // Copy over integrity information. Not sure if this is required.
        let mut dest_integrity_info = ffi::FSCTL_SET_INTEGRITY_INFORMATION_BUFFER {
            ChecksumAlgorithm: src_integrity_info.ChecksumAlgorithm,
            Reserved: src_integrity_info.Reserved,
            Flags: src_integrity_info.Flags,
        };
        dest.set_integrity_information(&mut dest_integrity_info)?;
    }

    // file_size must be sufficient to hold the data.
    // TODO test if the current implementation works:
    // Later on, we round up the bytes to copy in order to end at a cluster boundary.
    // This might very well result in us cloning past the file end.
    // Let's hope windows api sanitizes this, because otherwise a clean implementation is not really possible.
    dest.set_len(src_file_size)?;

    // Preparation done, now reflink
    let mut dup_extent: ffi::DUPLICATE_EXTENTS_DATA = unsafe { mem::uninitialized() };
    dup_extent.FileHandle = src.as_raw_handle();

    // We must end at a cluster boundary
    let total_copy_len: i64 = {
        if cluster_size == 0 {
            src_file_size as i64
        } else {
            // Round to the next cluster size
            round_up(src_file_size as i64, cluster_size)
        }
    };

    let mut bytes_copied = 0;
    // Must be smaller than 4GB; This is always a multiple of ClusterSize
    let max_copy_len: i64 = if cluster_size == 0 {
        total_copy_len
    } else {
        (4 * 1024 * 1024 * 1024) - cluster_size
    };
    while bytes_copied < total_copy_len {
        let bytes_to_copy = cmp::min(total_copy_len, max_copy_len);
        if cluster_size != 0 {
            debug_assert_eq!(bytes_to_copy % cluster_size, 0);
            debug_assert_eq!(bytes_copied % cluster_size, 0);
        }
        unsafe {
            *dup_extent.SourceFileOffset.QuadPart_mut() = bytes_copied;
            *dup_extent.TargetFileOffset.QuadPart_mut() = bytes_copied;
            *dup_extent.ByteCount.QuadPart_mut() = bytes_to_copy;
        }
        let mut bytes_returned = 0u32;
        let res = unsafe {
            DeviceIoControl(
                dest.as_raw_handle() as _,
                ffi::FSCTL_DUPLICATE_EXTENTS_TO_FILE,
                &mut dup_extent as *mut _ as *mut _,
                mem::size_of::<ffi::DUPLICATE_EXTENTS_DATA>() as u32,
                ptr::null_mut(),
                0,
                &mut bytes_returned as *mut _,
                ptr::null_mut(),
            )
        };
        if res == 0 {
            let _ = fs::remove_file(to);
            return Err(io::Error::last_os_error());
        }
        bytes_copied += bytes_to_copy;
    }
    dest.persist();
    Ok(())
}

/// Additional functionality for windows files, needed for reflink
trait FileExt {
    fn set_sparse(&self) -> io::Result<()>;
    fn get_integrity_information(&self) -> io::Result<ffi::FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>;
    fn set_integrity_information(
        &self,
        integrity_info: &mut ffi::FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()>;
    fn is_block_cloning_supported(&self) -> io::Result<bool>;
}

impl AsRawHandle for NamedTempFile {
    fn as_raw_handle(&self) -> RawHandle {
        self.inner.as_raw_handle()
    }
}

impl FileExt for NamedTempFile {
    fn set_sparse(&self) -> io::Result<()> {
        let mut bytes_returned = 0u32;
        let res = unsafe {
            DeviceIoControl(
                self.as_raw_handle() as _,
                FSCTL_SET_SPARSE,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
                &mut bytes_returned as *mut _,
                ptr::null_mut(),
            )
        };
        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn get_integrity_information(&self) -> io::Result<ffi::FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> {
        let mut bytes_returned = 0u32;
        unsafe {
            let mut integrity_info: ffi::FSCTL_GET_INTEGRITY_INFORMATION_BUFFER = mem::zeroed();
            let res = DeviceIoControl(
                self.as_raw_handle() as _,
                FSCTL_GET_INTEGRITY_INFORMATION,
                ptr::null_mut(),
                0,
                &mut integrity_info as *mut _ as *mut _,
                mem::size_of::<ffi::FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>() as u32,
                &mut bytes_returned as *mut _,
                ptr::null_mut(),
            );
            if res == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(integrity_info)
            }
        }
    }

    fn set_integrity_information(
        &self,
        integrity_info: &mut ffi::FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()> {
        let res = unsafe {
            DeviceIoControl(
                self.as_raw_handle() as _,
                FSCTL_SET_INTEGRITY_INFORMATION,
                integrity_info as *mut _ as *mut _,
                mem::size_of::<ffi::FSCTL_SET_INTEGRITY_INFORMATION_BUFFER>() as u32,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn is_block_cloning_supported(&self) -> io::Result<bool> {
        let mut flags = 0u32;
        let res = unsafe {
            GetVolumeInformationByHandleW(
                self.as_raw_handle() as _,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut flags as *mut _,
                ptr::null_mut(),
                0,
            )
        };
        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            println!("{:b}", flags);
            if flags & FILE_SUPPORTS_BLOCK_REFCOUNTING > 0 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

/// Rounds `num_to_round` to the next multiple of `multiple`, if `mutliple is a power of 2`
fn round_up(num_to_round: i64, multiple: i64) -> i64 {
    assert!(multiple != 0 && ((multiple & (multiple - 1)) == 0));
    (num_to_round + multiple - 1) & -multiple
}

/// Contains definitions not included in winapi
#[allow(non_snake_case)]
mod ffi {
    use std::os::windows::raw::HANDLE;
    use winapi::shared::{
        minwindef::{DWORD, WORD},
        ntdef::LARGE_INTEGER,
    };

    pub const FSCTL_DUPLICATE_EXTENTS_TO_FILE: u32 = 0x98344;

    #[derive(Debug)]
    #[repr(C)]
    pub struct FSCTL_GET_INTEGRITY_INFORMATION_BUFFER {
        pub ChecksumAlgorithm: WORD,
        pub Reserved: WORD,
        pub Flags: DWORD,
        pub ChecksumChunkSizeInBytes: DWORD,
        pub ClusterSizeInBytes: DWORD,
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct FSCTL_SET_INTEGRITY_INFORMATION_BUFFER {
        pub ChecksumAlgorithm: WORD,
        pub Reserved: WORD,
        pub Flags: DWORD,
    }

    #[repr(C)]
    pub struct DUPLICATE_EXTENTS_DATA {
        pub FileHandle: HANDLE,
        pub SourceFileOffset: LARGE_INTEGER,
        pub TargetFileOffset: LARGE_INTEGER,
        pub ByteCount: LARGE_INTEGER,
    }
}
