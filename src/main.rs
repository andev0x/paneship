#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: std::alloc::System = std::alloc::System;

mod benchmark;
mod cache;
mod core;
mod daemon;
mod detectors;
mod ipc;
mod modules;
#[cfg(unix)]
mod tmux;
mod top;
mod tui;

use std::path::PathBuf;

use benchmark::BenchmarkOptions;
use core::prompt::PromptContext;

#[derive(Debug, Clone)]
struct RenderOptions {
    exit_code: i32,
    width: Option<usize>,
    cwd: Option<PathBuf>,
    duration_ms: Option<u64>,
    shell: RenderShell,
    layout: RenderLayout,
}

/// Output shape requested by the shell adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderLayout {
    /// Left and right regions on one line, cursor on the next.
    Combined,
    /// `left <US> right <LF> cursor`, letting native right prompts
    /// (`RPROMPT`/`RPS1`, `fish_right_prompt`) own the alignment.
    Split,
}

/// Field separator used by [`RenderLayout::Split`] (ASCII unit separator).
const RPROMPT_SEPARATOR: char = '\u{1f}';

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

#[derive(Debug, Clone)]
#[cfg_attr(not(unix), allow(dead_code))]
enum CliCommand {
    Init(InitOptions),
    Render(RenderOptions),
    Benchmark(BenchmarkOptions),
    Configure,
    Top(top::TopOptions),
    Daemon,
    DaemonPing,
    DaemonReload,
    Invalidate(PathBuf),
    Version,
    Help,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(unix), allow(dead_code))]
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
            let cwd = options
                .cwd
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let width = options.width.unwrap_or(80);

            let parts = daemon::render(cwd, options.exit_code, width, options.duration_ms)
                .unwrap_or_else(|| {
                    let context = PromptContext::from_inputs(
                        options.cwd,
                        options.width,
                        options.exit_code,
                        options.duration_ms,
                    );
                    core::renderer::render_parts(&context)
                });

            let prompt = match options.layout {
                RenderLayout::Combined => {
                    apply_shell_wrapping(core::renderer::combine(&parts, width), options.shell)
                }
                RenderLayout::Split => render_split(parts, options.shell),
            };
            print!("{prompt}");
        }
        CliCommand::Benchmark(options) => {
            #[cfg(unix)]
            {
                match benchmark::run(options) {
                    Ok(report) => {
                        println!("{report}");
                    }
                    Err(err) => {
                        eprintln!("benchmark failed: {err}");
                        std::process::exit(1);
                    }
                }
            }
            #[cfg(not(unix))]
            {
                let _ = options;
                eprintln!("benchmark is only supported on Unix");
                std::process::exit(1);
            }
        }
        CliCommand::Top(options) => {
            if let Err(err) = top::run(&options) {
                eprintln!("top failed: {err}");
                std::process::exit(1);
            }
        }
        CliCommand::Configure => {
            if let Err(err) = tui::run() {
                eprintln!("config error: {err}");
                std::process::exit(1);
            }
        }
        CliCommand::Daemon => {
            if let Err(err) = daemon::run() {
                eprintln!("daemon error: {err}");
                std::process::exit(1);
            }
        }
        CliCommand::DaemonPing => {
            if daemon::ping() {
                println!("ok");
            } else {
                std::process::exit(1);
            }
        }
        CliCommand::DaemonReload => {
            if daemon::reload_config() {
                println!("ok");
            } else {
                std::process::exit(1);
            }
        }
        CliCommand::Invalidate(cwd) => {
            if !daemon::invalidate(&cwd) {
                std::process::exit(1);
            }
        }
        CliCommand::Init(options) => {
            #[cfg(unix)]
            {
                handle_init(options);
            }
            #[cfg(not(unix))]
            {
                let _ = options;
                eprintln!("init is only supported on Unix shells");
                std::process::exit(1);
            }
        }
        CliCommand::Help => {
            println!("{}", usage());
        }
        CliCommand::Version => {
            println!("paneship {}", env!("CARGO_PKG_VERSION"));
        }
    }
}

/// Apply the shell-specific ANSI escaping (zsh `%{ %}`, bash `\[ \]`).
fn apply_shell_wrapping(prompt: String, shell: RenderShell) -> String {
    match shell {
        RenderShell::Zsh => core::layout::wrap_ansi_for_zsh(prompt.as_str()),
        RenderShell::Bash => core::layout::wrap_ansi_for_bash(prompt.as_str()),
        _ => prompt,
    }
}

/// Emit the structured regions for native right prompts.
///
/// Wire format: `left \u{1f} right \n cursor`. Each region is ANSI-wrapped for
/// the target shell *before* joining, so a right-aligned `RPROMPT` measures
/// correctly even in emulators that do not understand bare escape sequences.
fn render_split(parts: core::renderer::PromptParts, shell: RenderShell) -> String {
    // Consume the regions instead of cloning them: the render path owns this
    // value and every byte here is on the prompt latency budget.
    let left = apply_shell_wrapping(parts.left, shell);
    let right = apply_shell_wrapping(parts.right, shell);
    let cursor = apply_shell_wrapping(parts.cursor, shell);
    format!("{left}{RPROMPT_SEPARATOR}{right}\n{cursor}")
}

fn parse_cli(args: Vec<String>) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Ok(CliCommand::Render(RenderOptions {
            exit_code: 0,
            width: None,
            cwd: None,
            duration_ms: None,
            shell: RenderShell::Plain,
            layout: RenderLayout::Combined,
        }));
    }

    if matches!(args[0].as_str(), "help" | "--help" | "-h") {
        return Ok(CliCommand::Help);
    }

    if matches!(args[0].as_str(), "version" | "--version" | "-V") {
        return Ok(CliCommand::Version);
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
        return Ok(CliCommand::Top(parse_top_args(&args[1..])?));
    }

    if args[0] == "config" {
        return Ok(CliCommand::Configure);
    }

    if args[0] == "daemon" {
        return parse_daemon_args(&args[1..]);
    }

    if args[0] == "invalidate" {
        return parse_invalidate_args(&args[1..]);
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
        "    case \"$PANESHIP_LAST_CMD\" in",
        "        git\\ *) \"$PANESHIP_BIN\" invalidate --cwd \"$PWD\" > /dev/null 2>&1 ;;",
        "    esac",
        "    PANESHIP_LAST_CMD=\"\"",
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
        "    PANESHIP_LAST_CMD=\"$BASH_COMMAND\"",
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
        "typeset -g PANESHIP_LAST_CMD=\"\"",
        "",
        "paneship_preexec() {",
        "    PANESHIP_CMD_START=$EPOCHREALTIME",
        "    PANESHIP_LAST_CMD=\"$1\"",
        "}",
        "",
        "paneship_precmd() {",
        "    local exit_code=$?",
        "    if [[ \"$PANESHIP_LAST_CMD\" == git\\ * ]]; then",
        "        \"$PANESHIP_BIN\" invalidate --cwd \"$PWD\" > /dev/null 2>&1",
        "    fi",
        "    PANESHIP_LAST_CMD=\"\"",
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
        "    rendered=\"$(\"$PANESHIP_BIN\" render --shell zsh --layout split --exit-code \"$exit_code\" --width \"${COLUMNS:-80}\" --cwd \"$PWD\" \"${duration_arg[@]}\" 2>/dev/null)\" || rendered=\"\"",
        "    if [[ -z \"$rendered\" || \"$rendered\" != *$'\\x1f'* ]]; then",
        "        PROMPT='%n@%m:%~ %# '",
        "        RPROMPT=''",
        "    else",
        "        local paneship_left=\"${rendered%%$'\\x1f'*}\"",
        "        local paneship_rest=\"${rendered#*$'\\x1f'}\"",
        "        local paneship_right=\"${paneship_rest%%$'\\n'*}\"",
        "        local paneship_cursor=\"${paneship_rest#*$'\\n'}\"",
        "        PROMPT=\"${paneship_left}",
        "${paneship_cursor}\"",
        "        RPROMPT=\"${paneship_right}\"",
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
        "function fish_preexec --on-event fish_preexec",
        "    set -g PANESHIP_LAST_CMD $argv",
        "end",
        "",
        "function fish_prompt",
        "    set -l exit_code $status",
        "    if test -n \"$PANESHIP_LAST_CMD\"; and string match -q 'git *' -- \"$PANESHIP_LAST_CMD\"",
        "        paneship invalidate --cwd $PWD >/dev/null 2>&1",
        "    end",
        "    set -g PANESHIP_LAST_CMD \"\"",
        "    set -l duration_arg",
        "    if test -n \"$CMD_DURATION\"",
        "        set duration_arg --duration-ms \"$CMD_DURATION\"",
        "    end",
        "    set -l rendered (paneship render --shell fish --layout split --exit-code $exit_code --width $COLUMNS --cwd $PWD $duration_arg | string collect)",
        "    if test -z \"$rendered\"; or not string match -q -- \"*\\x1f*\" \"$rendered\"",
        "        set -g PANESHIP_RPROMPT \"\"",
        "        printf '%s' '> '",
        "        return",
        "    end",
        "    set -l halves (string split \"\\x1f\" -- \"$rendered\")",
        "    set -l right_and_cursor (string split \"\\n\" -- \"$halves[2]\")",
        "    set -g PANESHIP_RPROMPT $right_and_cursor[1]",
        "    set -l cursor (string join \"\\n\" -- $right_and_cursor[2..-1])",
        "    printf '%s\\n%s' \"$halves[1]\" \"$cursor\"",
        "end",
        "",
        "function fish_right_prompt",
        "    if set -q PANESHIP_RPROMPT",
        "        printf '%s' \"$PANESHIP_RPROMPT\"",
        "    end",
        "end",
    ]
    .join("\n")
}

#[cfg(unix)]
fn powershell_init_script() -> String {
    [
        "if (!(Get-Command paneship -ErrorAction SilentlyContinue)) { return }",
        "if (!(paneship daemon ping)) { Start-Process paneship -ArgumentList \"daemon\" -NoNewWindow }",
        "$global:PaneshipGitCommand = $false",
        "if (Get-Module -ListAvailable -Name PSReadLine) {",
        "    try {",
        "        Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {",
        "            $line = $null; $cursor = $null",
        "            [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)",
        "            if ($line -match '^\\s*git\\s') { $global:PaneshipGitCommand = $true } else { $global:PaneshipGitCommand = $false }",
        "            [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()",
        "        }",
        "    } catch {}",
        "}",
        "function prompt {",
        "    $lastExitCode = if ($null -eq $?) { 0 } else { [int](-not $?) }",
        "    if ($global:PaneshipGitCommand) {",
        "        try { & paneship invalidate --cwd $PWD 2>$null | Out-Null } catch {}",
        "        $global:PaneshipGitCommand = $false",
        "    }",
        "    & paneship render --shell powershell --exit-code $lastExitCode --width $Host.UI.RawUI.WindowSize.Width --cwd $PWD",
        "}",
    ]
    .join("\n")
}

#[cfg(unix)]
fn nushell_init_script() -> String {
    [
        "$env.PANESHIP_LAST_CMD = \"\"",
        "$env.PANESHIP_RPROMPT = \"\"",
        "$env.config = ($env.config? | default {})",
        "$env.config.hooks = ($env.config.hooks? | default {})",
        "$env.config.hooks.pre_execution = (($env.config.hooks.pre_execution? | default []) | append {||",
        "    let line = (try { commandline } catch { \"\" })",
        "    $env.PANESHIP_LAST_CMD = ($line | default \"\" | into string)",
        "})",
        "$env.PROMPT_COMMAND = { ||",
        "    if ($env.PANESHIP_LAST_CMD | default \"\" | str starts-with \"git \") {",
        "        ^paneship invalidate --cwd $env.PWD | ignore",
        "    }",
        "    $env.PANESHIP_LAST_CMD = \"\"",
        "    let rendered = (^paneship render --shell nushell --layout split --exit-code $env.LAST_EXIT_CODE --width (term size).columns --cwd $env.PWD)",
        "    let halves = ($rendered | split row (char -i 0x1f))",
        "    if ($halves | length) < 2 {",
        "        $env.PANESHIP_RPROMPT = \"\"",
        "        return $rendered",
        "    }",
        "    let right_and_cursor = ($halves.1 | split row \"\\n\")",
        "    $env.PANESHIP_RPROMPT = ($right_and_cursor.0 | default \"\")",
        "    [$halves.0, ($right_and_cursor | skip 1 | str join \"\\n\")] | str join \"\\n\"",
        "}",
        "$env.PROMPT_COMMAND_RIGHT = { || $env.PANESHIP_RPROMPT }",
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
        layout: RenderLayout::Combined,
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
            "--layout" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --layout".to_string())?;
                options.layout = match value.as_str() {
                    "combined" => RenderLayout::Combined,
                    "split" | "rprompt" => RenderLayout::Split,
                    _ => {
                        return Err(format!(
                            "invalid layout value: {value}. Supported values: combined, split"
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

fn parse_top_args(args: &[String]) -> Result<top::TopOptions, String> {
    let mut options = top::TopOptions::default();
    for arg in args {
        match arg.as_str() {
            "--once" | "-1" => options.once = true,
            "--json" => options.json = true,
            "--benchmark" | "--local" => options.benchmark = true,
            unknown => return Err(format!("unknown top argument: {unknown}")),
        }
    }
    // JSON is always a single-shot snapshot.
    if options.json {
        options.once = true;
    }
    Ok(options)
}

fn parse_daemon_args(args: &[String]) -> Result<CliCommand, String> {
    match args {
        [] => Ok(CliCommand::Daemon),
        [subcommand] if subcommand == "ping" => Ok(CliCommand::DaemonPing),
        [subcommand] if subcommand == "reload" => Ok(CliCommand::DaemonReload),
        [unknown, ..] => Err(format!("unknown daemon argument: {unknown}")),
    }
}

fn parse_invalidate_args(args: &[String]) -> Result<CliCommand, String> {
    let mut cwd: Option<PathBuf> = None;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--cwd" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --cwd".to_string())?;
                cwd = Some(PathBuf::from(value));
            }
            unknown => {
                return Err(format!("unknown invalidate argument: {unknown}"));
            }
        }
        idx += 1;
    }

    Ok(CliCommand::Invalidate(cwd.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_default()
    })))
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
    "Paneship - high-performance shell prompt\n\nUSAGE:\n  paneship [render] [--exit-code <code>] [--width <cols>] [--cwd <path>] [--duration-ms <ms>] [--shell <name>] [--layout <combined|split>]\n  paneship init <shell> [--onboarding|to onboarding]\n  paneship benchmark [--iterations <n>] [--panes <n>] [--compare-starship] [--width <cols>] [--cwd <path>] [--exit-code <code>]\n  paneship top [--once] [--json] [--benchmark]\n  paneship config\n  paneship daemon [ping|reload]\n  paneship invalidate [--cwd <path>]\n  paneship version\n  paneship help\n\nSHELLS:\n  bash, zsh, fish, powershell, nushell, elvish, xonsh, tcsh, ion, cmd\n\nOPTIONS:\n  -s, --exit-code <code>    Last command exit code\n  -w, --width <cols>        Prompt width budget\n      --cwd <path>          Directory to render the prompt for\n      --duration-ms <ms>    Last command duration in milliseconds\n      --shell <name>        Prompt output mode (default: plain)\n      --layout <mode>       combined (default) or split (native RPROMPT)\n\nINIT OPTIONS:\n  paneship init <shell>     Print shell init script (for eval)\n  paneship init <shell> to onboarding\n                            Append paneship block to shell config file\n  paneship init <shell> --onboarding\n                            Same as 'to onboarding'\n\nBENCHMARK OPTIONS:\n  -n, --iterations <n>      Renders per pane (default: 200)\n  -p, --panes <n>           Number of concurrent panes (default: 4)\n      --compare-starship    Include direct Starship comparison\n\nTOP OPTIONS:\n      --once                Print one daemon telemetry snapshot and exit\n      --json                Emit the telemetry snapshot as JSON\n      --benchmark           Run the local component benchmark instead"
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn version_flag_is_recognised_in_every_spelling() {
        for arg in ["version", "--version", "-V"] {
            let command = parse_cli(vec![arg.to_string()]).expect("version should parse");
            assert!(
                matches!(command, CliCommand::Version),
                "{arg} should map to Version"
            );
        }
    }

    #[test]
    fn help_and_empty_args_are_handled() {
        assert!(matches!(
            parse_cli(vec!["--help".to_string()]).unwrap(),
            CliCommand::Help
        ));
        assert!(matches!(
            parse_cli(Vec::new()).unwrap(),
            CliCommand::Render(_)
        ));
    }

    #[test]
    fn nushell_script_binds_native_right_prompt() {
        let script = nushell_init_script();
        assert!(
            script.contains("--layout split"),
            "nushell must request the structured RPROMPT layout"
        );
        assert!(
            script.contains("PROMPT_COMMAND_RIGHT"),
            "nushell must bind PROMPT_COMMAND_RIGHT"
        );
        assert!(script.contains("PANESHIP_RPROMPT"));
    }

    #[test]
    fn zsh_and_fish_scripts_use_split_layout() {
        assert!(zsh_init_script().contains("--layout split"));
        assert!(fish_init_script().contains("--layout split"));
        assert!(zsh_init_script().contains("RPROMPT"));
        assert!(fish_init_script().contains("fish_right_prompt"));
    }
}
