fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rustc = std::env::var_os("RUSTC").ok_or("RUSTC is unavailable")?;
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()?;
    if !version.status.success() {
        return Err("could not determine build compiler version".into());
    }
    println!(
        "cargo:rustc-env=BENCH_BUILD_RUSTC={}",
        String::from_utf8(version.stdout)?.trim()
    );
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    tonic_build::compile_protos("proto/users.proto")?;
    println!("cargo:rerun-if-changed=proto/users.proto");
    Ok(())
}
