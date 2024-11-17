#![cfg(windows)]

use reflink_copy::{
    check_reflink_support, reflink, reflink_block, reflink_or_copy, ReflinkSupport,
};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const FILE_SIZE: usize = 256 * 1024;
const FILENAME: &str = "test_file.dat";

// paths are defined in build.yml

fn temp_dir() -> PathBuf {
    PathBuf::from(std::env::var("RUNNER_TEMP").expect("RUNNER_TEMP is not set"))
}
fn refs1_dir() -> PathBuf {
    temp_dir().join("dev-drives").join("refs1")
}
fn refs2_dir() -> PathBuf {
    temp_dir().join("dev-drives").join("refs2")
}
fn ntfs_dir() -> PathBuf {
    temp_dir().join("dev-drives").join("ntfs")
}

fn make_subfolder(folder: &Path, line: u32) -> std::io::Result<PathBuf> {
    let subfolder = folder.join(format!("subfolder_{line}"));
    std::fs::create_dir_all(&subfolder)?;
    Ok(subfolder)
}

fn create_test_file(path: &Path) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }

    let mut file = std::fs::File::create(path)?;
    file.write_all(&vec![0u8; FILE_SIZE])?;
    Ok(())
}

#[test]
#[ignore]
fn test_correct_deployment() {
    assert!(temp_dir().join("dev-drives").join("ntfs.vhdx").exists());
}

#[test]
#[ignore]
fn test_reflink_support_refs1_to_refs2() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), refs2_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_ntfs_to_refs1() -> std::io::Result<()> {
    let result = check_reflink_support(ntfs_dir(), refs1_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&ntfs_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_refs1_to_ntfs() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), ntfs_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&ntfs_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_refs1() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), refs1_dir())?;
    assert_eq!(result, ReflinkSupport::Supported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::Supported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_on_supported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    reflink(from.join(FILENAME), to.join(FILENAME))
}

#[test]
#[ignore]
fn test_reflink_on_unsupported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let _ = reflink(from.join(FILENAME), to.join(FILENAME)).unwrap_err();
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_or_copy_on_supported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let result = reflink_or_copy(from.join(FILENAME), to.join(FILENAME))?;
    assert_eq!(result, None);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_or_copy_on_unsupported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let result = reflink_or_copy(from.join(FILENAME), to.join(FILENAME))?;
    assert_eq!(result, Some(FILE_SIZE as u64));
    Ok(())
}

fn read_file_to_vec(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}
fn compare_files_eq(file1: &Path, file2: &Path) -> std::io::Result<()> {
    let data1 = read_file_to_vec(file1)?;
    let data2 = read_file_to_vec(file2)?;
    assert_eq!(data1, data2);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_block_whole_file() -> std::io::Result<()> {
    let cluster_size = 4096u64;
    let num_clusters = 3;
    let data_size = cluster_size * num_clusters;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&from)?;

    let data: Vec<u8> = (1..=num_clusters)
        .flat_map(|i| vec![i as u8; cluster_size as usize])
        .collect();
    source_file.write_all(&data)?;
    source_file.flush()?;

    let mut dest_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&to)?;

    dest_file.set_len(data_size as u64)?;
    reflink_block(&source_file, 0, &dest_file, 0, data_size)?;

    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    compare_files_eq(&from, &to)?;
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_block_reverse() -> std::io::Result<()> {
    let cluster_size = 4096u64;
    let num_clusters = 3;
    let data_size = cluster_size * num_clusters;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&from)?;

    let data: Vec<Vec<u8>> = (1..=num_clusters)
        .map(|i| vec![i as u8; cluster_size as usize])
        .collect();
    for cluster in &data {
        source_file.write_all(&cluster)?;
    }
    source_file.flush()?;

    let mut dest_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&to)?;

    dest_file.set_len(data_size as u64)?;

    for i in 0..num_clusters {
        let r = num_clusters - 1 - i;
        reflink_block(
            &source_file,
            i * cluster_size,
            &dest_file,
            r * cluster_size,
            cluster_size,
        )?;
    }
    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    let mut dest_file = std::fs::OpenOptions::new().read(true).open(&to)?;

    let mut buf = vec![0; cluster_size as usize];
    for i in num_clusters - 1..=0 {
        dest_file.read(buf.as_mut_slice())?;
        assert_eq!(buf, data[i as usize]);
    }

    Ok(())
}
