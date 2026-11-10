mod catalogue;
mod cli;
mod commands;
mod config;
mod diff;
mod envelope;
mod ops;
mod preview;
mod render;
mod rules;
mod spec;
mod spec_edit;
mod svg;
mod util;

use std::io::{IsTerminal, Write};

use clap::Parser;
use clap::error::{ContextKind, ContextValue, ErrorKind};

use cli::{Cli, Command, Format};
use envelope::{AppError, EXIT_INTERNAL, EXIT_OK, Output};

fn default_format() -> Format {
    if std::io::stdout().is_terminal() { Format::Text } else { Format::Json }
}

/// Best guess at --format when the arguments themselves did not parse.
fn format_from_args(args: &[String]) -> Format {
    for (i, a) in args.iter().enumerate() {
        let v = match a.strip_prefix("--format=") {
            Some(v) => Some(v),
            None if a == "--format" => args.get(i + 1).map(|s| s.as_str()),
            None => None,
        };
        match v {
            Some("json") => return Format::Json,
            Some("text") => return Format::Text,
            _ => {}
        }
    }
    default_format()
}

fn emit(format: Format, out: &Output) {
    let mut stdout = std::io::stdout().lock();
    let _ = match (&out.raw, format) {
        (Some(raw), _) => stdout.write_all(raw),
        (None, Format::Json) => writeln!(stdout, "{}", out.json),
        (None, Format::Text) => write!(stdout, "{}", out.text),
    };
    let _ = stdout.flush();
}

fn fail(format: Format, command: &str, err: &AppError) -> i32 {
    match format {
        Format::Json => {
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{}", err.to_json(command));
        }
        Format::Text => eprintln!("{}", err.to_text()),
    }
    err.exit
}

fn usage_error(e: clap::Error, args: &[String]) -> AppError {
    let rendered = e.render().to_string();
    let first = rendered.lines().next().unwrap_or("invalid arguments").trim_start_matches("error: ").trim().to_string();
    let message = if first.ends_with('.') { first } else { format!("{first}.") };
    let ctx_str = |kind: ContextKind| match e.get(kind) {
        Some(ContextValue::String(s)) => Some(s.clone()),
        Some(ContextValue::Strings(v)) => v.first().cloned(),
        _ => None,
    };
    let mut suggestion = ctx_str(ContextKind::SuggestedArg)
        .or_else(|| ctx_str(ContextKind::SuggestedSubcommand))
        .or_else(|| ctx_str(ContextKind::SuggestedValue))
        .map(|s| format!("Did you mean '{s}'?"));
    let code = match e.kind() {
        ErrorKind::UnknownArgument => "unknown-flag",
        ErrorKind::InvalidSubcommand => "unknown-command",
        ErrorKind::MissingRequiredArgument | ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => "missing-argument",
        ErrorKind::ArgumentConflict => "argument-conflict",
        _ => "invalid-argument",
    };
    let mut message = message;
    if e.kind() == ErrorKind::InvalidSubcommand
        && let Some(cmd) = args.iter().find(|a| cli::PLANNED.contains(&a.as_str()))
    {
        message = format!("The '{cmd}' command is specified but not built yet; this build has {}.", cli::COMMANDS.join(", "));
        suggestion = None;
    }
    if e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        message = format!("No command given; use one of {}.", cli::COMMANDS.join(", "));
    }
    if suggestion.is_none() && code != "unknown-command" {
        suggestion = Some("Run with --help to see the accepted arguments.".into());
    }
    AppError::usage(code, message).suggest(suggestion)
}

fn run(args: &[String]) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                print!("{}", e.render());
                return EXIT_OK;
            }
            let command = args.iter().skip(1).find(|a| cli::COMMANDS.contains(&a.as_str())).map(|s| s.as_str()).unwrap_or("svgdoc");
            return fail(format_from_args(args), command, &usage_error(e, args));
        }
    };
    let format = cli.global.format.unwrap_or_else(default_format);
    let name = cli.command.name();
    let color = format == Format::Text && std::io::stdout().is_terminal() && !cli.global.no_color && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty());

    let result = (|| -> Result<Output, AppError> {
        let profile = match &cli.global.profile {
            Some(p) => Some(
                config::Profile::parse(p)
                    .ok_or_else(|| AppError::usage("unknown-profile", format!("Unknown profile '{p}'.")).suggest(util::did_you_mean(p, config::PROFILES.iter().copied())))?,
            ),
            None => None,
        };
        let paint = commands::validate::Paint { color };
        let mut loader = match &cli.command {
            Command::Validate { .. } | Command::Edit { .. } | Command::Fix { .. } | Command::Render { .. } | Command::Preview { .. } => Some(config::Loader::new(cli.global.config.as_deref(), cli.global.no_config)?),
            _ => None,
        };
        let out = match &cli.command {
            Command::Validate { files, rules, skip, fail_on, max_findings } => commands::validate::run(files, rules, skip, *fail_on, *max_findings, loader.as_mut().unwrap(), profile, &paint),
            Command::Edit { file, set, ops, dry_run, output, in_place } => commands::edit::run(file, set, ops.as_deref(), *dry_run, output.as_deref(), *in_place, loader.as_mut().unwrap(), profile, &paint),
            Command::Fix { file, rules, allow_unsafe, dry_run, output, in_place } => commands::fix::run(file, rules, *allow_unsafe, *dry_run, output.as_deref(), *in_place, loader.as_mut().unwrap(), profile, &paint),
            Command::Render { spec, output, theme, no_validate, force, text_as_paths } => commands::render::run(spec, output, theme.as_deref(), *no_validate, *force, *text_as_paths, loader.as_mut().unwrap(), profile, &paint),
            Command::Preview { file, output, width, background, annotate, crop, force } => commands::preview::run(file, output, *width, *background, *annotate, crop.as_deref(), *force, loader.as_mut().unwrap(), profile, &paint),
            Command::Inspect { file, select, depth, fields } => commands::inspect::run(file, select.as_deref(), *depth, fields),
            Command::Schema { name } => commands::schema::run(*name),
            Command::Rules { explain } => commands::rules::run(explain.as_deref()),
        }?;
        if let Some(loader) = &loader
            && !cli.global.quiet
        {
            for p in &loader.loaded {
                eprintln!("svgdoc: config {}", p.display());
            }
        }
        Ok(out)
    })();

    match result {
        Ok(out) => {
            emit(format, &out);
            out.exit
        }
        Err(e) => fail(format, name, &e),
    }
}

fn main() {
    let args: Vec<String> = std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    // A panic is a bug: report it in the contract's shape instead of a backtrace.
    std::panic::set_hook(Box::new(|_| {}));
    let code = match std::panic::catch_unwind(|| run(&args)) {
        Ok(code) => code,
        Err(payload) => {
            let detail = payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".into());
            let err = AppError::new(EXIT_INTERNAL, "internal-error", format!("svgdoc hit a bug ({detail}). Please report it with the command and the input file."));
            let command = args.iter().skip(1).find(|a| cli::COMMANDS.contains(&a.as_str())).map(|s| s.as_str()).unwrap_or("svgdoc");
            fail(format_from_args(&args), command, &err)
        }
    };
    std::process::exit(code);
}
