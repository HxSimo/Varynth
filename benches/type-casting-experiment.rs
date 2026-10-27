//! Arrow cast microbenchmark. See benches/README.md for protocol and examples.

mod type_casting;

use std::{
    env,
    io::{self, Write},
    process::{Command, ExitCode},
};
use type_casting::{
    config::{Args, USAGE},
    report::{HEADER, Metadata},
};

const CHILD_RUN: &str = "VARYNTH_CAST_BENCH_CHILD_RUN";

fn execute() -> Result<(), String> {
    let items: Vec<String> = env::args().skip(1).collect();
    // Cargo builds harness-free benchmarks with cfg(test) for both commands,
    // but only `cargo bench` supplies --bench. Tests live in the integration target.
    if cfg!(test) && !items.iter().any(|arg| arg == "--bench") {
        return Ok(());
    }
    if items.iter().any(|arg| arg == "--help" || arg == "-h") {
        eprintln!("usage: {USAGE}");
        return Ok(());
    }
    let cfg = Args::parse(items.clone())?;
    let child_run = env::var(CHILD_RUN)
        .ok()
        .map(|text| {
            text.parse::<usize>()
                .map_err(|_| format!("invalid internal {CHILD_RUN}"))
        })
        .transpose()?;
    if child_run.is_none() {
        println!("{HEADER}");
        io::stdout().flush().map_err(|error| error.to_string())?;
    }
    if cfg.runs > 1 && child_run.is_none() {
        let executable = env::current_exe().map_err(|error| error.to_string())?;
        for run_index in 0..cfg.runs {
            let status = Command::new(&executable)
                .args(&items)
                .env(CHILD_RUN, run_index.to_string())
                .status()
                .map_err(|error| format!("cannot start run {run_index}: {error}"))?;
            if !status.success() {
                return Err(format!("run {run_index} failed: {status}"));
            }
        }
        return Ok(());
    }
    let metadata = Metadata::collect(child_run.unwrap_or(0));
    type_casting::run(
        &cfg,
        &metadata,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
    .map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}\nusage: {USAGE}");
            ExitCode::from(2)
        }
    }
}
