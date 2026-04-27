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
    duration_ms: Option<u64>,
}

#[cfg(unix)]
#[derive(Debug, Clone)]
enum CliCommand {
    Init(InitOptions),
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

#[cfg(unix)]
#[derive(Debug, Clone)]
enum InitOptions {
    Zsh { onboarding: bool },
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
            let context = PromptContext::from_inputs(
                options.cwd,
                options.width,
                options.exit_code,
                options.duration_ms,
            );
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
        #[cfg(unix)]
        CliCommand::Init(options) => {
            handle_init(options);
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
            duration_ms: None,
        }));
    }

    if matches!(args[0].as_str(), "help" | "--help" | "-h") {
        return Ok(CliCommand::Help);
    }

    if args[0] == "benchmark" {
        #[cfg(unix)]
        {
            return parse_benchmark_args(&args[1..]);
        }
        #[cfg(not(unix))]
        {
            return Err("benchmark command is only available on Unix".to_string());
        }
    }

    #[cfg(unix)]
    if args[0] == "daemon" {
        return Ok(CliCommand::Daemon);
    }

    #[cfg(unix)]
    if args[0] == "init" {
        return parse_init_args(&args[1..]);
    }

    if args[0] == "render" {
        return parse_render_args(&args[1..]);
    }

    parse_render_args(&args)
}

#[cfg(unix)]
fn handle_init(options: InitOptions) {
    match options {
        InitOptions::Zsh { onboarding } => {
            let script = zsh_init_script();
            if onboarding {
                if let Err(err) = append_zsh_onboarding(&script) {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
                return;
            }

            print!("{script}");
        }
    }
}

#[cfg(unix)]
fn zsh_init_script() -> String {
    [
        "if ! pgrep -x \"paneship\" > /dev/null; then",
        "    paneship daemon > /dev/null 2>&1 &",
        "    disown",
        "fi",
        "",
        "zmodload zsh/datetime",
        "autoload -Uz add-zsh-hook",
        "typeset -gF PANESHIP_CMD_START=0",
        "",
        "paneship_preexec() {",
        "    PANESHIP_CMD_START=$EPOCHREALTIME",
        "}",
        "",
        "paneship_precmd() {",
        "    if (( PANESHIP_CMD_START > 0 )); then",
        "        local -F now=$EPOCHREALTIME",
        "        local -F elapsed=$(( now - PANESHIP_CMD_START ))",
        "        local elapsed_ms=$(( elapsed * 1000 ))",
        "        if (( elapsed_ms < 0 )); then",
        "            elapsed_ms=0",
        "        fi",
        "        export PANESHIP_LAST_CMD_DURATION_MS=$elapsed_ms",
        "    else",
        "        unset PANESHIP_LAST_CMD_DURATION_MS",
        "    fi",
        "}",
        "",
        "add-zsh-hook preexec paneship_preexec",
        "add-zsh-hook precmd paneship_precmd",
        "",
        "PROMPT='$(paneship render --exit-code $? --width $COLUMNS)'",
    ]
    .join("\n")
}

#[cfg(unix)]
fn append_zsh_onboarding(script: &str) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let home = std::env::var("HOME").map_err(|_| "Unable to find $HOME".to_string())?;
    let zshrc_path = format!("{home}/.zshrc");

    let start_marker = "# >>> paneship initialize >>>";
    let end_marker = "# <<< paneship initialize <<<";
    let block = format!("{start_marker}\n{script}\n{end_marker}\n");

    let existing = std::fs::read_to_string(&zshrc_path).unwrap_or_default();
    if existing.contains(start_marker)
        || existing.contains("paneship render --exit-code $? --width $COLUMNS")
    {
        println!("Paneship onboarding is already configured in {zshrc_path}");
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&zshrc_path)
        .map_err(|err| format!("Failed to open {zshrc_path}: {err}"))?;

    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file).map_err(|err| format!("Failed to write to {zshrc_path}: {err}"))?;
    }

    write!(file, "{block}").map_err(|err| format!("Failed to write to {zshrc_path}: {err}"))?;

    println!("Paneship onboarding config appended to {zshrc_path}");
    println!("Restart your shell or run: source {zshrc_path}");

    Ok(())
}

fn parse_render_args(args: &[String]) -> Result<CliCommand, String> {
    let mut options = RenderOptions {
        exit_code: 0,
        width: None,
        cwd: None,
        duration_ms: None,
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
            "--duration-ms" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --duration-ms".to_string())?;
                options.duration_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid duration ms: {value}"))?,
                );
            }
            unknown => {
                return Err(format!("unknown render argument: {unknown}"));
            }
        }
        idx += 1;
    }

    Ok(CliCommand::Render(options))
}

#[cfg(unix)]
fn parse_benchmark_args(args: &[String]) -> Result<CliCommand, String> {
    let mut options = crate::benchmark::BenchmarkOptions::default();
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

#[cfg(unix)]
fn parse_init_args(args: &[String]) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Err("missing init target. Try: paneship init zsh".to_string());
    }

    if args[0] != "zsh" {
        return Err(format!(
            "unsupported init target '{}'. Currently only 'zsh' is supported.",
            args[0]
        ));
    }

    let onboarding = match args.get(1).map(|v| v.as_str()) {
        None => false,
        Some("--onboarding") => true,
        Some("to") if args.get(2).map(|v| v.as_str()) == Some("onboarding") && args.len() == 3 => {
            true
        }
        Some("onboarding") if args.len() == 2 => true,
        Some(_) => {
            return Err(
                "unsupported init syntax. Use: paneship init zsh [--onboarding|to onboarding]"
                    .to_string(),
            )
        }
    };

    Ok(CliCommand::Init(InitOptions::Zsh { onboarding }))
}

fn usage() -> &'static str {
    "Paneship - high-performance shell prompt\n\nUSAGE:\n  paneship [render] [--exit-code <code>] [--width <cols>] [--cwd <path>] [--duration-ms <ms>]\n  paneship init zsh [--onboarding|to onboarding]\n  paneship benchmark [--iterations <n>] [--panes <n>] [--compare-starship] [--width <cols>] [--cwd <path>] [--exit-code <code>]\n  paneship daemon\n  paneship help\n\nOPTIONS:\n  -s, --exit-code <code>    Last command exit code\n  -w, --width <cols>        Prompt width budget\n      --cwd <path>          Directory to render the prompt for\n      --duration-ms <ms>    Last command duration in milliseconds\n\nINIT OPTIONS:\n  paneship init zsh         Print zsh init script (for eval)\n  paneship init zsh to onboarding\n                            Append paneship block to ~/.zshrc\n  paneship init zsh --onboarding\n                            Same as 'to onboarding'\n\nBENCHMARK OPTIONS:\n  -n, --iterations <n>      Renders per pane (default: 200)\n  -p, --panes <n>           Number of concurrent panes (default: 4)\n      --compare-starship    Include direct Starship comparison"
}
