use std::process::Command;

#[test]
fn generated_code_compiles_and_runs_against_runtime() {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generated_project/Cargo.toml");
    let target =
        std::env::temp_dir().join(format!("morphrpc-generated-test-{}", std::process::id()));
    let status = Command::new(env!("CARGO"))
        .args(["test", "--locked", "--manifest-path"])
        .arg(manifest)
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("run generated-code integration test");
    let _ = std::fs::remove_dir_all(target);
    assert!(status.success());
}
