mod preview;

use clap::Parser;
use preview::PreviewServer;
use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

#[derive(Debug, Parser)]
#[command(version, after_help = "Press Ctrl+C to stop the preview server.")]
/// Preview a Markdown presentation.
struct Cli {
    /// Markdown deck to preview.
    #[arg(value_name = "DECK.md")]
    deck: PathBuf,

    /// Open the preview in Chromium app mode instead of the default browser.
    #[arg(long, conflicts_with = "no_open")]
    app: bool,

    /// Print the preview URL without opening a browser.
    #[arg(long)]
    no_open: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("flik: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = env::current_dir()?.join(cli.deck);
    let preview = PreviewServer::bind(path)?;
    let url = preview.url();
    println!("Preview: {url}\nPress Ctrl+C to stop.");
    if !cli.no_open {
        launch_preview(url, cli.app);
    }
    preview.serve();
    Ok(())
}

fn launch_preview(url: String, app: bool) {
    let (program, arg) = if app {
        ("chromium", format!("--app={url}"))
    } else {
        ("xdg-open", url.clone())
    };
    match Command::new(program)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut opener) => {
            std::thread::spawn(move || {
                if !opener.wait().is_ok_and(|status| status.success()) {
                    eprintln!("{program} did not launch successfully. Open {url} manually.");
                }
            });
        }
        Err(err) => eprintln!("Could not start {program}: {err}. Open {url} manually."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn cli_parses_launch_modes() {
        let cli = Cli::try_parse_from(["flik", "examples/demo.md"]).unwrap();
        assert_eq!(cli.deck, Path::new("examples/demo.md"));
        assert!(!cli.app && !cli.no_open);
        assert!(
            Cli::try_parse_from(["flik", "--app", "deck.md"])
                .unwrap()
                .app
        );
        assert!(
            Cli::try_parse_from(["flik", "deck.md", "--no-open"])
                .unwrap()
                .no_open
        );
        let conflict = Cli::try_parse_from(["flik", "--app", "--no-open", "deck.md"]).unwrap_err();
        assert_eq!(conflict.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}
