#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use clap::Parser;
    match erc4626_balance_state_tools::run(erc4626_balance_state_tools::Cli::parse()) {
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
