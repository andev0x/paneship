mod benchmark;
mod cache;
mod core;
#[cfg(unix)]
mod daemon;
mod modules;
#[cfg(unix)]
mod tmux;

use std::path::PathBuf;

#[cfg(unix)]
use benchmark::BenchmarkOptions;
use core::prompt::PromptContext;

#[derive(Debug, Clone)]
struct RenderOptions {
    exit_code: i32,
    width: Option<usize>,
    cwd: Option<PathBuf>,
}

#[cfg(unix)]
#[derive(Debug, Clone)]
enum CliCommand {
    Render(RenderOptions),
    Benchmark(BenchmarkOptions),
    Daemon,
    Help,
}

#[cfg(not(unix))]
#[derive(Debug, Clone)]
enum CliCommand {
    Render(RenderOptions),
    Help,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse_cli(args) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("{err}\n\n{}", usage());
            std::process::exit(2);
        }
    };

    match command {
        CliCommand::Render(options) => {
            let context = PromptContext::from_inputs(options.cwd, options.width, options.exit_code);
            let prompt = core::renderer::render(&context);
            print!("{prompt}");
        }
        #[cfg(unix)]
        CliCommand::Benchmark(options) => match benchmark::run(options) {
            Ok(report) => {
                println!("{report}");
            }
            Err(err) => {
                eprintln!("benchmark failed: {err}");
                std::process::exit(1);
            }
        },
        #[cfg(unix)]
        CliCommand::Daemon => {
            if let Err(err) = daemon::run() {
                eprintln!("daemon error: {err}");
                std::process::exit(1);
            }
        }
        CliCommand::Help => {
            println!("{}", usage());
        }
    }
}

fn parse_cli(args: Vec<String>) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Ok(CliCommand::Render(RenderOptions {
            exit_code: 0,
            width: None,
            cwd: None,
        }));
    }

    if matches!(args[0].as_str(), "help" | "--help" | "-h") {
        return Ok(CliCommand::Help);
    }

    if args[0] == "benchmark" {
        #[cfg(unix)]
        return parse_benchmark_args(&args[1..]);
        #[cfg(not(unix))]
        return Err("benchmark command is only available on Unix".to_string());
    }

    #[cfg(unix)]
    if args[0] == "daemon" {
        return Ok(CliCommand::Daemon);
    }

    if args[0] == "render" {
        return parse_render_args(&args[1..]);
    }

    parse_render_args(&args)
}

fn parse_render_args(args: &[String]) -> Result<CliCommand, String> {
    let mut options = RenderOptions {
        exit_code: 0,
        width: None,
        cwd: None,
    };

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--exit-code" | "-s" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --exit-code".to_string())?;
                options.exit_code = value
                    .parse::<i32>()
                    .map_err(|_| format!("invalid exit code: {value}"))?;
            }
            "--width" | "-w" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --width".to_string())?;
                options.width = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid width: {value}"))?,
                );
            }
            "--cwd" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --cwd".to_string())?;
                options.cwd = Some(PathBuf::from(value));
            }
            unknown => {
                return Err(format!("unknown render argument: {unknown}"));
            }
        }
        idx += 1;
    }

    Ok(CliCommand::Render(options))
}

fn parse_benchmark_args(args: &[String]) -> Result<CliCommand, String> {
    let mut options = BenchmarkOptions::default();
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--iterations" | "-n" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --iterations".to_string())?;
                options.iterations = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid iterations value: {value}"))?;
            }
            "--panes" | "-p" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --panes".to_string())?;
                options.panes = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid panes value: {value}"))?;
            }
            "--compare-starship" => {
                options.compare_starship = true;
            }
            "--width" | "-w" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --width".to_string())?;
                options.width = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid width: {value}"))?,
                );
            }
            "--cwd" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --cwd".to_string())?;
                options.cwd = Some(PathBuf::from(value));
            }
            "--exit-code" | "-s" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --exit-code".to_string())?;
                options.exit_code = value
                    .parse::<i32>()
                    .map_err(|_| format!("invalid exit code: {value}"))?;
            }
            unknown => {
                return Err(format!("unknown benchmark argument: {unknown}"));
            }
        }
        idx += 1;
    }

    if options.iterations == 0 {
        return Err("iterations must be greater than 0".to_string());
    }
    if options.panes == 0 {
        return Err("panes must be greater than 0".to_string());
    }

    Ok(CliCommand::Benchmark(options))
}

fn usage() -> &'static str {
    "Paneship - high-performance shell prompt\n\nUSAGE:\n  paneship [render] [--exit-code <code>] [--width <cols>] [--cwd <path>]\n  paneship benchmark [--iterations <n>] [--panes <n>] [--compare-starship] [--width <cols>] [--cwd <path>] [--exit-code <code>]\n  paneship daemon\n  paneship help\n\nOPTIONS:\n  -s, --exit-code <code>    Last command exit code\n  -w, --width <cols>        Prompt width budget\n      --cwd <path>          Directory to render the prompt for\n\nBENCHMARK OPTIONS:\n  -n, --iterations <n>      Renders per pane (default: 200)\n  -p, --panes <n>           Number of concurrent panes (default: 4)\n      --compare-starship    Include direct Starship comparison"
}
