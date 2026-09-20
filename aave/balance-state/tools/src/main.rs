#[cfg(not(target_arch = "wasm32"))]
fn main() {
    match aave_balance_state_tools::cli::run() {
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
