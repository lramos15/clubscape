fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    config.include_file("clubscape.rs");
    config.file_descriptor_set_path(
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("account-descriptor.bin"),
    );
    config.compile_protos(&["proto/game.proto", "proto/account.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/account.proto");
    println!("cargo:rerun-if-changed=proto/game.proto");
    Ok(())
}
