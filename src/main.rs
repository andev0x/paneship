#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: std::alloc::System = std::alloc::System;

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
    shell: RenderShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderShell {
    Plain,
    Zsh,
}

#[cfg(unix)]
#[derive(Debug, Clone)]
enum CliCommand {
    Init(InitOptions),
    Render(RenderOptions),
    Benchmark(BenchmarkOptions),
    Top,
    Daemon,
    DaemonPing,
    Help,
}

#[cfg(not(unix))]
#[derive(Debug, Clone)]
enum CliCommand {
    Render(RenderOptions),
    Top,
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
            #[cfg(unix)]
            {
                let cwd = options
                    .cwd
                    .clone()
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let width = options.width.unwrap_or(80);
                if let Some(mut prompt) =
                    daemon::render(cwd, options.exit_code, width, options.duration_ms)
                {
                    if matches!(options.shell, RenderShell::Zsh) {
                        prompt = core::layout::wrap_ansi_for_zsh(prompt.as_str());
                    }
                    print!("{prompt}");
                    return;
                }
            }

            let context = PromptContext::from_inputs(
                options.cwd,
                options.width,
                options.exit_code,
                options.duration_ms,
            );
            let mut prompt = core::renderer::render(&context);
            if matches!(options.shell, RenderShell::Zsh) {
                prompt = core::layout::wrap_ansi_for_zsh(prompt.as_str());
            }
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
        CliCommand::Top => {
            if let Err(err) = benchmark::run_top() {
                eprintln!("top benchmark failed: {err}");
                std::process::exit(1);
            }
        }
        #[cfg(unix)]
        CliCommand::Daemon => {
            if let Err(err) = daemon::run() {
                eprintln!("daemon error: {err}");
                std::process::exit(1);
            }
        }
        #[cfg(unix)]
        CliCommand::DaemonPing => {
            if daemon::ping() {
                println!("ok");
            } else {
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
            shell: RenderShell::Plain,
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

    if matches!(args[0].as_str(), "top" | "-top") {
        return Ok(CliCommand::Top);
    }

    #[cfg(unix)]
    if args[0] == "daemon" {
        return parse_daemon_args(&args[1..]);
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
        "if (( ! $+commands[paneship] )); then",
        "    return 0 2>/dev/null || true",
        "fi",
        "typeset -g PANESHIP_BIN=\"${commands[paneship]}\"",
        "",
        "if ! \"$PANESHIP_BIN\" daemon ping > /dev/null 2>&1; then",
        "    \"$PANESHIP_BIN\" daemon > /dev/null 2>&1 &!",
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
        "    local exit_code=$?",
        "    local -a duration_arg",
        "    if (( PANESHIP_CMD_START > 0 )); then",
        "        local -F now=$EPOCHREALTIME",
        "        local -F elapsed=$(( now - PANESHIP_CMD_START ))",
        "        local -i elapsed_ms=$(( elapsed * 1000 ))",
        "        if (( elapsed_ms < 0 )); then",
        "            elapsed_ms=0",
        "        fi",
        "        duration_arg=(--duration-ms \"$elapsed_ms\")",
        "    fi",
        "    local rendered",
        "    rendered=\"$(\"$PANESHIP_BIN\" render --shell zsh --exit-code \"$exit_code\" --width \"${COLUMNS:-80}\" --cwd \"$PWD\" \"${duration_arg[@]}\" 2>/dev/null)\" || rendered=\"\"",
        "    if [[ -z \"$rendered\" || \"$rendered\" != *$'\\n'* || \"$rendered\" == *$'\\n'*$'\\n'* ]]; then",
        "        PROMPT='%n@%m:%~ %# '",
        "    else",
        "        PROMPT=\"$rendered\"",
        "    fi",
        "    PANESHIP_CMD_START=0",
        "}",
        "",
        "add-zsh-hook -D preexec paneship_preexec 2>/dev/null",
        "add-zsh-hook -D precmd paneship_precmd 2>/dev/null",
        "add-zsh-hook preexec paneship_preexec",
        "add-zsh-hook precmd paneship_precmd",
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
    if let Some(start_idx) = existing.find(start_marker) {
        let Some(rel_end_idx) = existing[start_idx..].find(end_marker) else {
            return Err(format!(
                "Found '{start_marker}' without matching '{end_marker}' in {zshrc_path}. Please fix this block manually."
            ));
        };

        let end_marker_idx = start_idx + rel_end_idx;
        let mut replace_end = end_marker_idx + end_marker.len();
        if existing[replace_end..].starts_with('\n') {
            replace_end += 1;
        }

        let mut updated = existing.clone();
        updated.replace_range(start_idx..replace_end, block.as_str());

        if updated == existing {
            println!("Paneship onboarding is already up to date in {zshrc_path}");
            return Ok(());
        }

        std::fs::write(&zshrc_path, updated)
            .map_err(|err| format!("Failed to write to {zshrc_path}: {err}"))?;

        println!("Paneship onboarding config updated in {zshrc_path}");
        println!("Restart your shell or run: source {zshrc_path}");
        return Ok(());
    }

    if existing.contains(block.trim_end()) {
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
        shell: RenderShell::Plain,
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
            "-h" | "--help" => {}
            "--shell" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --shell".to_string())?;
                options.shell = match value.as_str() {
                    "zsh" => RenderShell::Zsh,
                    "plain" => RenderShell::Plain,
                    _ => {
                        return Err(format!(
                            "invalid shell value: {value}. Supported values: plain, zsh"
                        ))
                    }
                };
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
fn parse_daemon_args(args: &[String]) -> Result<CliCommand, String> {
    match args {
        [] => Ok(CliCommand::Daemon),
        [subcommand] if subcommand == "ping" => Ok(CliCommand::DaemonPing),
        [unknown, ..] => Err(format!("unknown daemon argument: {unknown}")),
    }
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
    "Paneship - high-performance shell prompt\n\nUSAGE:\n  paneship [render] [--exit-code <code>] [--width <cols>] [--cwd <path>] [--duration-ms <ms>] [--shell <plain|zsh>]\n  paneship init zsh [--onboarding|to onboarding]\n  paneship benchmark [--iterations <n>] [--panes <n>] [--compare-starship] [--width <cols>] [--cwd <path>] [--exit-code <code>]\n  paneship top\n  paneship daemon [ping]\n  paneship help\n\nOPTIONS:\n  -s, --exit-code <code>    Last command exit code\n  -w, --width <cols>        Prompt width budget\n      --cwd <path>          Directory to render the prompt for\n      --duration-ms <ms>    Last command duration in milliseconds\n      --shell <name>        Prompt output mode: plain or zsh\n\nINIT OPTIONS:\n  paneship init zsh         Print zsh init script (for eval)\n  paneship init zsh to onboarding\n                            Append paneship block to ~/.zshrc\n  paneship init zsh --onboarding\n                            Same as 'to onboarding'\n\nBENCHMARK OPTIONS:\n  -n, --iterations <n>      Renders per pane (default: 200)\n  -p, --panes <n>           Number of concurrent panes (default: 4)\n      --compare-starship    Include direct Starship comparison"
}
