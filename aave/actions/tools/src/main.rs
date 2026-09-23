#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use clap::Parser;
    let args = aave_actions_tools::Cli::parse();
    match aave_actions_tools::run(args) {
        Ok(true) => (),
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
