use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    match foundry_mcp::serve(stdin.lock(), stdout.lock(), stderr.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("foundry-mcp: {err}");
            ExitCode::from(1)
        }
    }
}
