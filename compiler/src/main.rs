use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    process::ExitCode,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: morphrpc-compiler CONTRACT.json > generated.rs".into());
    }
    let mut input = String::new();
    File::open(&args[1])?
        .take((morphrpc_compiler::MAX_CONTRACT_BYTES + 1) as u64)
        .read_to_string(&mut input)?;
    let contract = morphrpc_compiler::parse(&input)?;
    io::stdout()
        .lock()
        .write_all(morphrpc_compiler::generate(&contract).as_bytes())?;
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
