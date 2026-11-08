use app_lib::commands::copy_file_inner as copy_file;
use std::fs;

#[test]
fn copy_creates_parent_dirs() {
    let tmp = std::env::temp_dir();
    let src = tmp.join("solomd-copy-src.png").to_string_lossy().to_string();
    let test_dir = tmp.join("solomd-copy-test");
    let dst = test_dir.join("_assets/nested/image.png").to_string_lossy().to_string();
    let _ = fs::remove_dir_all(&test_dir);
    let bytes: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
    fs::write(&src, &bytes).unwrap();

    copy_file(src, dst.clone()).unwrap();

    let copied = fs::read(&dst).unwrap();
    assert_eq!(copied, vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn copy_overwrites_existing() {
    let tmp = std::env::temp_dir();
    let src = tmp.join("solomd-copy-src2.png").to_string_lossy().to_string();
    let dst = tmp.join("solomd-copy-dst2.png").to_string_lossy().to_string();
    fs::write(&src, b"new").unwrap();
    fs::write(&dst, b"old").unwrap();

    copy_file(src, dst.clone()).unwrap();

    assert_eq!(fs::read(&dst).unwrap(), b"new");
}

#[test]
fn copy_missing_source_errors() {
    let tmp = std::env::temp_dir();
    let src = tmp.join("solomd-does-not-exist.png").to_string_lossy().to_string();
    let dst = tmp.join("solomd-dst-fail.png").to_string_lossy().to_string();
    let result = copy_file(src, dst);
    assert!(result.is_err());
}

#[test]
fn write_binary_creates_parent_dirs() {
    use app_lib::commands::write_binary_file_inner as write_binary_file;
    let tmp = std::env::temp_dir();
    let test_dir = tmp.join("solomd-bin-test");
    let _ = fs::remove_dir_all(&test_dir);
    let path = test_dir.join("sub/dir/file.bin").to_string_lossy().to_string();
    write_binary_file(path.clone(), vec![1, 2, 3, 4]).unwrap();
    assert_eq!(fs::read(&path).unwrap(), vec![1, 2, 3, 4]);
}
