mod cli;
mod interpreter;
mod yaml;

fn main() {
    let exit_code = match cli::run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {:#?}", e);
            1
        }
    };

    std::process::exit(exit_code);
}
