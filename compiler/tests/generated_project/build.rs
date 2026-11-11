fn main() {
    let schema = std::fs::read_to_string("contract.json").unwrap();
    let generated = morphrpc_compiler::generate(&morphrpc_compiler::parse(&schema).unwrap());
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("generated.rs");
    std::fs::write(output, generated).unwrap();
    let schema = r#"{"schema_version":1,"service":"Empty","messages":[{"name":"EmptyRequest","fields":[]}],"methods":[{"name":"empty","operation_id":1,"request":"EmptyRequest","response":"EmptyRequest"}]}"#;
    let generated = morphrpc_compiler::generate(&morphrpc_compiler::parse(schema).unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("empty.rs");
    std::fs::write(output, generated).unwrap();
    println!("cargo:rerun-if-changed=contract.json");
}
