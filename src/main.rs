use clap::Parser;

fn main() {
    let cli = xretype::cli::Cli::parse();

    if let Err(error) = xretype::run(cli) {
        eprintln!("xretype: {error:#}");
        std::process::exit(1);
    }
}
