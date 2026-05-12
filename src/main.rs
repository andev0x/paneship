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
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Nushell,
    Elvish,
    Xonsh,
    Tcsh,
    Ion,
    Cmd,
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
struct InitOptions {
    shell: RenderShell,
    onboarding: bool,
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
                    match options.shell {
                        RenderShell::Zsh => {
                            prompt = core::layout::wrap_ansi_for_zsh(prompt.as_str())
                        }
                        RenderShell::Bash => {
                            prompt = core::layout::wrap_ansi_for_bash(prompt.as_str())
                        }
                        _ => {}
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
            match options.shell {
                RenderShell::Zsh => prompt = core::layout::wrap_ansi_for_zsh(prompt.as_str()),
                RenderShell::Bash => prompt = core::layout::wrap_ansi_for_bash(prompt.as_str()),
                _ => {}
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
    let script = match options.shell {
        RenderShell::Bash => bash_init_script(),
        RenderShell::Zsh => zsh_init_script(),
        RenderShell::Fish => fish_init_script(),
        RenderShell::PowerShell => powershell_init_script(),
        RenderShell::Nushell => nushell_init_script(),
        RenderShell::Elvish => elvish_init_script(),
        RenderShell::Xonsh => xonsh_init_script(),
        RenderShell::Tcsh => tcsh_init_script(),
        RenderShell::Ion => ion_init_script(),
        RenderShell::Cmd => cmd_init_script(),
        RenderShell::Plain => "".to_string(),
    };

    if options.onboarding {
        if let Err(err) = append_onboarding(options.shell, &script) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }

    print!("{script}");
}

#[cfg(unix)]
fn bash_init_script() -> String {
    [
        "if ! command -v paneship >/dev/null 2>&1; then",
        "    return 0",
        "fi",
        "PANESHIP_BIN=$(command -v paneship)",
        "",
        "if ! \"$PANESHIP_BIN\" daemon ping > /dev/null 2>&1; then",
        "    \"$PANESHIP_BIN\" daemon > /dev/null 2>&1 &",
        "fi",
        "",
        "paneship_precmd() {",
        "    local exit_code=$?",
        "    local duration_arg=()",
        "    if [[ -n \"$PANESHIP_START_TIME\" ]]; then",
        "        local end_time=$(date +%s%3N)",
        "        local elapsed=$((end_time - PANESHIP_START_TIME))",
        "        duration_arg=(--duration-ms \"$elapsed\")",
        "    fi",
        "    PS1=\"$(\"$PANESHIP_BIN\" render --shell bash --exit-code \"$exit_code\" --width \"${COLUMNS:-80}\" --cwd \"$PWD\" \"${duration_arg[@]}\" 2>/dev/null)\"",
        "    unset PANESHIP_START_TIME",
        "}",
        "",
        "paneship_preexec() {",
        "    PANESHIP_START_TIME=$(date +%s%3N)",
        "}",
        "",
        "if [[ \";$PROMPT_COMMAND;\" != *\";paneship_precmd;\"* ]]; then",
        "    PROMPT_COMMAND=\"paneship_precmd; $PROMPT_COMMAND\"",
        "fi",
        "",
        "trap 'paneship_preexec' DEBUG",
    ]
    .join("\n")
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
fn fish_init_script() -> String {
    [
        "if not command -v paneship >/dev/null 2>&1",
        "    exit",
        "end",
        "",
        "if not paneship daemon ping >/dev/null 2>&1",
        "    paneship daemon >/dev/null 2>&1 &",
        "    disown",
        "end",
        "",
        "function fish_prompt",
        "    set -l exit_code $status",
        "    set -l duration_arg",
        "    if test -n \"$CMD_DURATION\"",
        "        set duration_arg --duration-ms \"$CMD_DURATION\"",
        "    end",
        "    paneship render --shell fish --exit-code $exit_code --width $COLUMNS --cwd $PWD $duration_arg",
        "end",
    ]
    .join("\n")
}

#[cfg(unix)]
fn powershell_init_script() -> String {
    [
        "function prompt {",
        "    $lastExitCode = if ($null -eq $?) { 0 } else { [int](-not $?) }",
        "    & paneship render --shell powershell --exit-code $lastExitCode --width $Host.UI.RawUI.WindowSize.Width --cwd $PWD",
        "}",
        "if (!(Get-Command paneship -ErrorAction SilentlyContinue)) { return }",
        "if (!(paneship daemon ping)) { Start-Process paneship -ArgumentList \"daemon\" -NoNewWindow }",
    ]
    .join("\n")
}

#[cfg(unix)]
fn nushell_init_script() -> String {
    [
        "$env.PROMPT_COMMAND = { ||",
        "    paneship render --shell nushell --exit-code $env.LAST_EXIT_CODE --width (term size).columns --cwd $env.PWD",
        "}",
        "$env.PROMPT_COMMAND_RIGHT = \"\"",
    ]
    .join("\n")
}

#[cfg(unix)]
fn elvish_init_script() -> String {
    [
        "set edit:prompt = {",
        "    paneship render --shell elvish --exit-code (if $edit:exceptions-visible { put 1 } else { put 0 }) --width (take 1 (stty size | from-spaced)) --cwd $pwd",
        "}",
    ]
    .join("\n")
}

#[cfg(unix)]
fn xonsh_init_script() -> String {
    [
        "$PROMPT = lambda: $(paneship render --shell xonsh --exit-code __xonsh__.history[-1].rtn if len(__xonsh__.history) > 0 else 0 --width $COLUMNS --cwd $PWD)",
    ]
    .join("\n")
}

#[cfg(unix)]
fn tcsh_init_script() -> String {
    [
        "alias precmd 'set prompt=\"`paneship render --shell tcsh --exit-code $status --width $COLUMNS --cwd $cwd`\"'",
    ]
    .join("\n")
}

#[cfg(unix)]
fn ion_init_script() -> String {
    [
        "fn PROMPT",
        "    paneship render --shell ion --exit-code $? --width $COLUMNS --cwd $PWD",
        "end",
    ]
    .join("\n")
}

#[cfg(unix)]
fn cmd_init_script() -> String {
    "rem paneship init for cmd.exe usually requires clink or similar enhancements.\nset PROMPT=$E[32mpaneship$E[0m $P$G ".to_string()
}

#[cfg(unix)]
fn append_onboarding(shell: RenderShell, script: &str) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let home = std::env::var("HOME").map_err(|_| "Unable to find $HOME".to_string())?;

    let config_path = match shell {
        RenderShell::Bash => format!("{home}/.bashrc"),
        RenderShell::Zsh => format!("{home}/.zshrc"),
        RenderShell::Fish => format!("{home}/.config/fish/config.fish"),
        RenderShell::PowerShell => {
            format!("{home}/.config/powershell/Microsoft.PowerShell_profile.ps1")
        }
        RenderShell::Nushell => format!("{home}/.config/nushell/config.nu"),
        RenderShell::Elvish => format!("{home}/.elvish/rc.elv"),
        RenderShell::Xonsh => format!("{home}/.xonshrc"),
        RenderShell::Tcsh => format!("{home}/.tcshrc"),
        RenderShell::Ion => format!("{home}/.config/ion/initrc"),
        _ => {
            return Err(format!(
                "Onboarding is not supported for shell: {:?}",
                shell
            ))
        }
    };

    let start_marker = "# >>> paneship initialize >>>";
    let end_marker = "# <<< paneship initialize <<<";
    let block = format!("{start_marker}\n{script}\n{end_marker}\n");

    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    if let Some(start_idx) = existing.find(start_marker) {
        let Some(rel_end_idx) = existing[start_idx..].find(end_marker) else {
            return Err(format!(
                "Found '{start_marker}' without matching '{end_marker}' in {config_path}. Please fix this block manually."
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
            println!("Paneship onboarding is already up to date in {config_path}");
            return Ok(());
        }

        std::fs::write(&config_path, updated)
            .map_err(|err| format!("Failed to write to {config_path}: {err}"))?;

        println!("Paneship onboarding config updated in {config_path}");
        return Ok(());
    }

    if existing.contains(block.trim_end()) {
        println!("Paneship onboarding is already configured in {config_path}");
        return Ok(());
    }

    if let Some(parent) = std::path::Path::new(&config_path).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!("Failed to create directory {}: {}", parent.display(), err)
            })?;
        }
    }

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&config_path)
        .map_err(|err| format!("Failed to open {config_path}: {err}"))?;

    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file).map_err(|err| format!("Failed to write to {config_path}: {err}"))?;
    }

    write!(file, "{block}").map_err(|err| format!("Failed to write to {config_path}: {err}"))?;

    println!("Paneship onboarding config appended to {config_path}");
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
                    "plain" => RenderShell::Plain,
                    "bash" => RenderShell::Bash,
                    "zsh" => RenderShell::Zsh,
                    "fish" => RenderShell::Fish,
                    "powershell" | "pwsh" => RenderShell::PowerShell,
                    "nushell" | "nu" => RenderShell::Nushell,
                    "elvish" => RenderShell::Elvish,
                    "xonsh" => RenderShell::Xonsh,
                    "tcsh" => RenderShell::Tcsh,
                    "ion" => RenderShell::Ion,
                    "cmd" => RenderShell::Cmd,
                    _ => {
                        return Err(format!(
                            "invalid shell value: {value}. Supported values: plain, bash, zsh, fish, powershell, nushell, elvish, xonsh, tcsh, ion, cmd"
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

    let shell = match args[0].as_str() {
        "bash" => RenderShell::Bash,
        "zsh" => RenderShell::Zsh,
        "fish" => RenderShell::Fish,
        "powershell" | "pwsh" => RenderShell::PowerShell,
        "nushell" | "nu" => RenderShell::Nushell,
        "elvish" => RenderShell::Elvish,
        "xonsh" => RenderShell::Xonsh,
        "tcsh" => RenderShell::Tcsh,
        "ion" => RenderShell::Ion,
        "cmd" => RenderShell::Cmd,
        _ => {
            return Err(format!(
                "unsupported init target '{}'. Supported: bash, zsh, fish, powershell, nushell, elvish, xonsh, tcsh, ion, cmd",
                args[0]
            ));
        }
    };

    let onboarding = match args.get(1).map(|v| v.as_str()) {
        None => false,
        Some("--onboarding") => true,
        Some("to") if args.get(2).map(|v| v.as_str()) == Some("onboarding") && args.len() == 3 => {
            true
        }
        Some("onboarding") if args.len() == 2 => true,
        Some(_) => {
            return Err(format!(
                "unsupported init syntax. Use: paneship init {} [--onboarding|to onboarding]",
                args[0]
            ))
        }
    };

    Ok(CliCommand::Init(InitOptions { shell, onboarding }))
}

fn usage() -> &'static str {
    "Paneship - high-performance shell prompt\n\nUSAGE:\n  paneship [render] [--exit-code <code>] [--width <cols>] [--cwd <path>] [--duration-ms <ms>] [--shell <name>]\n  paneship init <shell> [--onboarding|to onboarding]\n  paneship benchmark [--iterations <n>] [--panes <n>] [--compare-starship] [--width <cols>] [--cwd <path>] [--exit-code <code>]\n  paneship top\n  paneship daemon [ping]\n  paneship help\n\nSHELLS:\n  bash, zsh, fish, powershell, nushell, elvish, xonsh, tcsh, ion, cmd\n\nOPTIONS:\n  -s, --exit-code <code>    Last command exit code\n  -w, --width <cols>        Prompt width budget\n      --cwd <path>          Directory to render the prompt for\n      --duration-ms <ms>    Last command duration in milliseconds\n      --shell <name>        Prompt output mode (default: plain)\n\nINIT OPTIONS:\n  paneship init <shell>     Print shell init script (for eval)\n  paneship init <shell> to onboarding\n                            Append paneship block to shell config file\n  paneship init <shell> --onboarding\n                            Same as 'to onboarding'\n\nBENCHMARK OPTIONS:\n  -n, --iterations <n>      Renders per pane (default: 200)\n  -p, --panes <n>           Number of concurrent panes (default: 4)\n      --compare-starship    Include direct Starship comparison"
}
