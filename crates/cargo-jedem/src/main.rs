//! `cargo jedem` — a thin front door to a crate's generator.
//!
//! # Why this is a wrapper and not the whole story
//!
//! A surface is `&'static` data inside the user's compiled crate. Nothing can
//! read it without building and running that crate, so generation is
//! necessarily a bin target; a cargo subcommand cannot conjure the descriptors
//! by inspecting source. What this removes is the need to *remember* the
//! invocation, and — with [`jedem::generator_main!`] — the fifteen lines of
//! identical `main()` that used to sit behind it.
//!
//!     cargo jedem generate
//!
//! runs the crate's `jedem-generate` bin, by convention.

use std::process::{Command, ExitCode};

const BIN: &str = "jedem-generate";

fn main() -> ExitCode {
    // Cargo invokes us as `cargo-jedem jedem <args>`; drop the repeated name.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .skip_while(|a| a == "jedem")
        .collect();

    match args.first().map(String::as_str) {
        Some("generate") => run_generator(&args[1..]),
        Some("help") | Some("--help") | Some("-h") | None => {
            help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("cargo jedem: unknown command `{other}`\n");
            help();
            ExitCode::FAILURE
        }
    }
}

fn run_generator(rest: &[String]) -> ExitCode {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.args(["run", "--quiet", "--bin", BIN]);
    if !rest.is_empty() {
        cmd.arg("--").args(rest);
    }
    match cmd.status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(_) => {
            eprintln!(
                "\ncargo jedem: `{BIN}` failed.\n\
                 If this crate has no generator yet, add one:\n\n\
                 \x20   // src/bin/{BIN}.rs\n\
                 \x20   jedem::generator_main! {{\n\
                 \x20       surface: my_crate::JEDEM_SURFACE,\n\
                 \x20       core: \"my_crate\",\n\
                 \x20       out: \"..\",\n\
                 \x20   }}\n"
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("cargo jedem: could not run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn help() {
    println!(
        "cargo jedem — project a Rust crate's functions into other languages\n\n\
         USAGE:\n\
         \x20   cargo jedem generate      run this crate's `{BIN}` bin\n\
         \x20   cargo jedem help\n\n\
         Generation runs a bin target because a surface is `&'static` data\n\
         inside your crate: reading it means running your crate.\n"
    );
}
