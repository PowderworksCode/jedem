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
//! runs the generation test the surface emits.

mod host;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    // Cargo invokes us as `cargo-jedem jedem <args>`; drop the repeated name.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .skip_while(|a| a == "jedem")
        .collect();

    match args.first().map(String::as_str) {
        Some("generate") => run_generator(&args[1..]),
        Some("run") => run_host(&args[1..]),
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

fn run_generator(_rest: &[String]) -> ExitCode {
    // Generation lives in the test `surface! { bindings: ... }` emits, so this
    // needs no bin target in the user's crate and nothing for them to write.
    // Writing is opt-in, which is why an ordinary `cargo test` is a check.
    let mut cmd = Command::new(host::cargo());
    cmd.args(["test", "--quiet", "__jedem_bindings"])
        .env("JEDEM_WRITE", "1");
    match cmd.status() {
        Ok(s) if s.success() => {
            println!("bindings written");
            ExitCode::SUCCESS
        }
        Ok(_) => {
            eprintln!(
                "\ncargo jedem generate: no surface found.\n\
                 Add `bindings:` to your `jedem::surface!` so it owns generation:\n\n\
                 \x20   jedem::surface! {{\n\
                 \x20       name: \"mycrate\", version: \"0.1.0\",\n\
                 \x20       api: [my_module],\n\
                 \x20       bindings: \"bindings\",\n\
                 \x20   }}\n"
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("cargo jedem generate: could not run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Build a binding and run a host program against it.
///
/// This exists because every consumer was otherwise writing the same eight
/// lines per language -- build the cdylib, find it, rename it to what the
/// runtime imports, put it on a path, run. jedem knows all of that.
fn run_host(args: &[String]) -> ExitCode {
    let mut target = None;
    let mut package = None;
    let mut rest: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--python" => target = Some("python"),
            "--node" => target = Some("node"),
            "--package" | "-p" => match it.next() {
                Some(p) => package = Some(p.clone()),
                None => {
                    eprintln!("cargo jedem run: --package needs a value");
                    return ExitCode::FAILURE;
                }
            },
            _ => {
                rest.push(a.clone());
                rest.extend(it.by_ref().cloned());
            }
        }
    }

    let Some(target) = target else {
        eprintln!("cargo jedem run: pass --python or --node\n");
        help();
        return ExitCode::FAILURE;
    };
    let Some((script, script_args)) = rest.split_first() else {
        eprintln!("cargo jedem run --{target}: give a script to run");
        return ExitCode::FAILURE;
    };

    let plan = match host::plan(package.as_deref(), target) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cargo jedem run: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Python imports `<module>`; node requires `<module>.node`. The module name
    // is the lib name with any target suffix removed.
    let module = plan
        .lib_name
        .strip_suffix(&format!("_{target}"))
        .unwrap_or(&plan.lib_name)
        .to_string();
    let extension = if target == "node" { "node" } else { "so" };

    let placed = match host::build_and_place(&plan, extension, &module) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cargo jedem run: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("placed {} (import `{module}`)", placed.display());

    let mut cmd = match target {
        "python" => {
            let mut c = Command::new("python3");
            c.env("PYTHONPATH", ".jedem");
            c
        }
        _ => Command::new("node"),
    };
    cmd.arg(script).args(script_args);
    match cmd.status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("cargo jedem run: could not start the host: {e}");
            ExitCode::FAILURE
        }
    }
}

fn help() {
    println!(
        "cargo jedem — project a Rust crate's functions into other languages\n\n\
         USAGE:\n\
         \x20   cargo jedem generate                  run this crate's the surface's generation test\n\
         \x20   cargo jedem run --python <script>     build the binding, run it\n\
         \x20   cargo jedem run --node <script>\n\
         \x20   cargo jedem help\n\n\
         `run` builds the binding crate, places its artefact in .jedem/ under\n\
         the name that runtime imports, and runs your script against it.\n\
         Python gets PYTHONPATH=.jedem; node requires ./.jedem/<module>.node\n\n\
         Generation runs a bin target because a surface is `&'static` data\n\
         inside your crate: reading it means running your crate.\n"
    );
}
