use super::utility::AutoRemovedFile;
use crate::ReflinkSupport;
use std::os::windows::ffi::OsStrExt;
use std::{
    convert::TryInto,
    ffi::c_void,
    fs::File,
    io,
    mem::{self, MaybeUninit},
    os::windows::{fs::MetadataExt, io::AsRawHandle},
    path::Path,
};

use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::{HANDLE, MAX_PATH},
    Storage::FileSystem::{
        GetVolumeInformationByHandleW, GetVolumeInformationW, FILE_ATTRIBUTE_SPARSE_FILE,
        FILE_FLAGS_AND_ATTRIBUTES,
    },
    System::{
        Ioctl::{
            DUPLICATE_EXTENTS_DATA, FSCTL_DUPLICATE_EXTENTS_TO_FILE,
            FSCTL_GET_INTEGRITY_INFORMATION, FSCTL_GET_INTEGRITY_INFORMATION_BUFFER,
            FSCTL_SET_INTEGRITY_INFORMATION, FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
            FSCTL_SET_SPARSE,
        },
        SystemServices::FILE_SUPPORTS_BLOCK_REFCOUNTING,
        IO::DeviceIoControl,
    },
};

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    // Inspired by https://github.com/0xbadfca11/reflink/blob/master/reflink.cpp
    let src = File::open(from)?;

    let src_metadata = src.metadata()?;
    let src_file_size = src_metadata.file_size();
    let src_is_sparse =
        (FILE_FLAGS_AND_ATTRIBUTES(src_metadata.file_attributes()) & FILE_ATTRIBUTE_SPARSE_FILE).0
            != 0;

    let dest = AutoRemovedFile::create_new(to)?;

    // Set the destination to be sparse while we clone.
    // Important to avoid allocating zero-backed real storage when cloning
    // below which will just be released when cloning file extents.
    dest.set_sparse()?;

    let src_integrity_info = src.get_integrity_information()?;
    let cluster_size: i64 = src_integrity_info.ClusterSizeInBytes.into();
    if cluster_size != 0 {
        if cluster_size != 4 * 1024 && cluster_size != 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cluster size of source must either be 4K or 64K (restricted by ReFS)",
            ));
        }
        // Copy over integrity information. Not sure if this is required.
        let mut dest_integrity_info = FSCTL_SET_INTEGRITY_INFORMATION_BUFFER {
            ChecksumAlgorithm: src_integrity_info.ChecksumAlgorithm,
            Reserved: src_integrity_info.Reserved,
            Flags: src_integrity_info.Flags,
        };

        // ignore the error if it fails, the clone will still work
        if let Err(_e) = dest.set_integrity_information(&mut dest_integrity_info) {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                ?_e,
                "Failed to set integrity information (probably on DevDriver), but the clone still works"
            );
        }
    }

    // file_size must be sufficient to hold the data.
    // TODO test if the current implementation works:
    // Later on, we round up the bytes to copy in order to end at a cluster boundary.
    // This might very well result in us cloning past the file end.
    // Let's hope windows api sanitizes this, because otherwise a clean implementation is not really possible.
    dest.as_inner_file().set_len(src_file_size)?;

    // We must end at a cluster boundary
    let total_copy_len: i64 = {
        if cluster_size == 0 {
            src_file_size.try_into().unwrap()
        } else {
            // Round to the next cluster size
            round_up(src_file_size.try_into().unwrap(), cluster_size)
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
        let bytes_to_copy = total_copy_len.min(max_copy_len);
        if cluster_size != 0 {
            debug_assert_eq!(bytes_to_copy % cluster_size, 0);
            debug_assert_eq!(bytes_copied % cluster_size, 0);
        }

        let mut dup_extent = DUPLICATE_EXTENTS_DATA {
            FileHandle: src.as_handle(),

            SourceFileOffset: bytes_copied,
            TargetFileOffset: bytes_copied,
            ByteCount: bytes_to_copy,
        };

        let mut bytes_returned = 0u32;
        unsafe {
            DeviceIoControl(
                dest.as_handle(),
                FSCTL_DUPLICATE_EXTENTS_TO_FILE,
                Some(&mut dup_extent as *mut _ as *mut c_void),
                mem::size_of::<DUPLICATE_EXTENTS_DATA>().try_into().unwrap(),
                None,
                0,
                Some(&mut bytes_returned as *mut _),
                None,
            )
        }?;
        bytes_copied += bytes_to_copy;
    }

    if !src_is_sparse {
        dest.unset_sparse()?;
    }

    dest.persist();
    Ok(())
}

/// Additional functionality for windows files, needed for reflink
trait FileExt {
    fn set_sparse(&self) -> io::Result<()>;
    fn unset_sparse(&self) -> io::Result<()>;
    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>;
    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()>;
    fn is_block_cloning_supported(&self) -> io::Result<bool>;

    fn as_handle(&self) -> HANDLE;
}

impl FileExt for File {
    fn set_sparse(&self) -> io::Result<()> {
        let mut bytes_returned = 0u32;
        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_SPARSE,
                None,
                0,
                None,
                0,
                Some(&mut bytes_returned as *mut _),
                None,
            )
        }?;

        Ok(())
    }

    fn unset_sparse(&self) -> io::Result<()> {
        let mut bytes_returned = 0u32;
        let mut sparse_flag: u32 = 0;

        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_SPARSE,
                Some(&mut sparse_flag as *mut _ as *mut c_void),
                mem::size_of::<u32>() as u32,
                None,
                0,
                Some(&mut bytes_returned as *mut _),
                None,
            )
        }?;

        Ok(())
    }

    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> {
        let mut bytes_returned = 0u32;
        let mut integrity_info: MaybeUninit<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> =
            MaybeUninit::uninit();

        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_GET_INTEGRITY_INFORMATION,
                None,
                0,
                Some(integrity_info.as_mut_ptr() as *mut c_void),
                mem::size_of::<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>()
                    .try_into()
                    .unwrap(),
                Some(&mut bytes_returned as *mut _),
                None,
            )?;

            Ok(integrity_info.assume_init())
        }
    }

    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()> {
        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_INTEGRITY_INFORMATION,
                Some(integrity_info as *mut _ as *mut c_void),
                mem::size_of::<FSCTL_SET_INTEGRITY_INFORMATION_BUFFER>()
                    .try_into()
                    .unwrap(),
                None,
                0,
                None,
                None,
            )
        }?;
        Ok(())
    }

    fn is_block_cloning_supported(&self) -> io::Result<bool> {
        let mut flags = 0u32;
        unsafe {
            GetVolumeInformationByHandleW(
                self.as_handle(),
                None,
                None,
                None,
                Some(&mut flags as *mut _),
                None,
            )
        }?;
        Ok((flags & FILE_SUPPORTS_BLOCK_REFCOUNTING) != 0)
    }

    fn as_handle(&self) -> HANDLE {
        HANDLE(self.as_raw_handle())
    }
}

impl FileExt for AutoRemovedFile {
    fn set_sparse(&self) -> io::Result<()> {
        self.as_inner_file().set_sparse()
    }

    fn unset_sparse(&self) -> io::Result<()> {
        self.as_inner_file().unset_sparse()
    }

    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> {
        self.as_inner_file().get_integrity_information()
    }

    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()> {
        self.as_inner_file()
            .set_integrity_information(integrity_info)
    }

    fn is_block_cloning_supported(&self) -> io::Result<bool> {
        self.as_inner_file().is_block_cloning_supported()
    }

    fn as_handle(&self) -> HANDLE {
        self.as_inner_file().as_handle()
    }
}

/// Rounds `num_to_round` to the next multiple of `multiple`
///
/// # Precondition
///  - `multiple` > 0
///  - `mutliple` is a power of 2
fn round_up(num_to_round: i64, multiple: i64) -> i64 {
    debug_assert!(multiple > 0);
    debug_assert_eq!((multiple & (multiple - 1)), 0);
    (num_to_round + multiple - 1) & -multiple
}

pub fn check_reflink_support<P, Q>(from: P, to: Q) -> io::Result<ReflinkSupport>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let from_info = get_volume_information(from)?;
    let to_info = get_volume_information(to)?;

    if from_info.supports_block_refcounting()
        && to_info.supports_block_refcounting()
        && from_info.volume_name == to_info.volume_name
    {
        Ok(ReflinkSupport::Supported)
    } else {
        Ok(ReflinkSupport::NotSupported)
    }
}

struct VolumeInformation {
    volume_name: String,
    file_system_flags: u32,
}

impl VolumeInformation {
    fn supports_block_refcounting(&self) -> bool {
        self.file_system_flags & FILE_SUPPORTS_BLOCK_REFCOUNTING == FILE_SUPPORTS_BLOCK_REFCOUNTING
    }
}

fn get_volume_information<P: AsRef<Path>>(path: P) -> io::Result<VolumeInformation> {
    // Convert the volume path to a wide string (PCWSTR)
    let volume_path_w: Vec<u16> = path
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0)) // Null terminator
        .collect();

    // Prepare buffers and variables for results
    let mut volume_name_buffer = vec![0u16; MAX_PATH as usize];
    let mut volume_name_len = MAX_PATH;
    let mut file_system_flags = 0u32;

    unsafe {
        GetVolumeInformationW(
            PCWSTR(volume_path_w.as_ptr()),
            Some(volume_name_buffer.as_mut()),
            Some(&mut volume_name_len),
            None,
            Some(&mut file_system_flags as *mut _),
            None,
        )
    }?;

    volume_name_buffer.resize(volume_name_len as usize, 0);
    Ok(VolumeInformation {
        volume_name: String::from_utf16_lossy(&volume_name_buffer),
        file_system_flags,
    })
}
