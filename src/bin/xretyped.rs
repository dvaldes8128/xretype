use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "xretyped",
    version,
    about = "Persistent xretype automation service"
)]
struct Args {
    /// Read settings from this TOML file.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let result =
        xretype::config::Config::load(args.config.as_deref()).and_then(xretype::daemon::serve);
    if let Err(error) = result {
        eprintln!("xretyped: {error:#}");
        std::process::exit(1);
    }
}
