mod args;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use foundry_api::{Command, Session};

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    let cli = match args::parse(&argv) {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            eprint!("{}", args::help_text());
            return ExitCode::from(2);
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: args::Cli) -> Result<ExitCode, String> {
    match cli {
        args::Cli::Help => {
            print!("{}", args::help_text());
            Ok(ExitCode::SUCCESS)
        }
        args::Cli::New { name, upm, out } => {
            let mut session = Session::new();
            let created = session.execute(Command::Create { name, upm });
            ensure_ok(created)?;
            let saved = session.execute(Command::Save {
                path: path_string(&out),
            });
            ensure_ok(saved)?;
            println!("Wrote {}", out.display());
            Ok(ExitCode::SUCCESS)
        }
        args::Cli::Info { path } => {
            let mut session = Session::new();
            let opened = session.execute(Command::Open {
                path: path_string(&path),
            });
            let data = ensure_ok(opened)?;
            print_info(&data);
            Ok(ExitCode::SUCCESS)
        }
        args::Cli::Check { a, b } => {
            let mut session = Session::new();
            let response = session.execute(Command::Check {
                a: path_string(&a),
                b: path_string(&b),
            });
            if response.ok {
                println!("Compatible.");
                Ok(ExitCode::SUCCESS)
            } else {
                print_failure(&response);
                Ok(ExitCode::from(1))
            }
        }
        args::Cli::Blend { a, b, t, out } => {
            let mut session = Session::new();
            let response = session.execute(Command::Blend {
                a: path_string(&a),
                b: path_string(&b),
                t,
                out: path_string(&out),
            });
            if response.ok {
                let data = response.data.unwrap_or(serde_json::Value::Null);
                println!(
                    "Wrote {}",
                    data["path"]
                        .as_str()
                        .unwrap_or(out.to_str().unwrap_or("the output"))
                );
                println!("Name: {}", data["name"].as_str().unwrap_or(""));
                Ok(ExitCode::SUCCESS)
            } else {
                print_failure(&response);
                Ok(ExitCode::from(1))
            }
        }
        args::Cli::Run { file } => run_stream(file),
    }
}

fn run_stream(file: Option<std::path::PathBuf>) -> Result<ExitCode, String> {
    let reader: Box<dyn BufRead> = if let Some(path) = file {
        let handle = fs::File::open(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        Box::new(io::BufReader::new(handle))
    } else {
        Box::new(io::BufReader::new(io::stdin()))
    };
    let mut session = Session::new();
    let mut failed = false;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in reader.lines() {
        let line = line.map_err(|err| err.to_string())?;
        let Some(response) = session.execute_line(&line) else {
            continue;
        };
        failed |= !response.ok;
        serde_json::to_writer(&mut out, &response).map_err(|err| err.to_string())?;
        writeln!(out).map_err(|err| err.to_string())?;
    }
    if failed {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn ensure_ok(response: foundry_api::Response) -> Result<serde_json::Value, String> {
    if response.ok {
        Ok(response.data.unwrap_or(serde_json::Value::Null))
    } else {
        Err(response
            .error
            .unwrap_or_else(|| "command failed".to_string()))
    }
}

fn print_info(data: &serde_json::Value) {
    println!("Name: {}", data["name"].as_str().unwrap_or(""));
    println!("UPM: {}", data["upm"].as_u64().unwrap_or(0));
    let glyphs = data["glyphs"].as_array().map(|items| {
        items
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });
    println!("Glyphs: {}", glyphs.unwrap_or_default());
}

fn print_failure(response: &foundry_api::Response) {
    if let Some(error) = &response.error {
        eprintln!("{error}");
    }
}

fn path_string(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
