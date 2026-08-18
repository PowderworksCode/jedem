//! `cargo jedem run` — build a binding and run a host program against it.
//!
//! Every consumer was writing the same eight lines per language: build the
//! cdylib, find it under `target/`, rename it to whatever that runtime expects
//! to import, put it on a path, run the script. Generating those scripts would
//! have been the wrong fix — jedem knows all of it, so the lines should stop
//! existing rather than be emitted.
//!
//! The artefact lands in `.jedem/` next to where you ran the command:
//!
//! | target | file | how the host finds it |
//! |---|---|---|
//! | python | `.jedem/<module>.so` | `PYTHONPATH=.jedem`, then `import <module>` |
//! | node | `.jedem/<module>.node` | `require("./.jedem/<module>.node")` |

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Plan {
    pub package: String,
    /// The cdylib cargo will emit, without the `lib` prefix or extension.
    pub lib_name: String,
    pub target_dir: PathBuf,
}

/// Ask cargo what it is going to build.
///
/// Reading the manifest ourselves would mean reimplementing workspace
/// resolution; `cargo metadata` already knows.
pub fn plan(package: Option<&str>, suffix: &str) -> Result<Plan, String> {
    let out = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("could not run `cargo metadata`: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("unreadable metadata: {e}"))?;

    let target_dir = meta["target_directory"]
        .as_str()
        .ok_or("metadata has no target_directory")?
        .into();
    let packages = meta["packages"]
        .as_array()
        .ok_or("metadata has no packages")?;

    // Either the package named, or the one conventionally named for this target.
    let chosen = packages
        .iter()
        .find(|p| match package {
            Some(want) => p["name"].as_str() == Some(want),
            None => p["name"]
                .as_str()
                .is_some_and(|n| n.ends_with(&format!("-{suffix}"))),
        })
        .ok_or_else(|| match package {
            Some(want) => format!("no package named `{want}` in this workspace"),
            None => format!(
                "no package named `*-{suffix}` in this workspace.\n\
                 Run `cargo jedem generate` first, or name one with --package."
            ),
        })?;

    let lib = chosen["targets"]
        .as_array()
        .and_then(|ts| {
            ts.iter().find(|t| {
                t["crate_types"]
                    .as_array()
                    .is_some_and(|cs| cs.iter().any(|c| c == "cdylib"))
            })
        })
        .ok_or_else(|| {
            format!(
                "package `{}` has no cdylib target",
                chosen["name"].as_str().unwrap_or("?")
            )
        })?;

    Ok(Plan {
        package: chosen["name"].as_str().unwrap_or_default().to_string(),
        lib_name: lib["name"].as_str().unwrap_or_default().replace('-', "_"),
        target_dir,
    })
}

/// Build the binding and place its artefact where the host will look.
/// Returns the module name the host should import.
pub fn build_and_place(plan: &Plan, extension: &str, module: &str) -> Result<PathBuf, String> {
    let status = Command::new(cargo())
        .args(["build", "--quiet", "-p", &plan.package])
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("`cargo build -p {}` failed", plan.package));
    }

    let built = ["so", "dylib", "dll"]
        .iter()
        .map(|ext| {
            plan.target_dir
                .join("debug")
                .join(format!("lib{}.{ext}", plan.lib_name))
        })
        .find(|p| p.exists())
        .or_else(|| {
            // Windows drops the `lib` prefix.
            let p = plan
                .target_dir
                .join("debug")
                .join(format!("{}.dll", plan.lib_name));
            p.exists().then_some(p)
        })
        .ok_or_else(|| {
            format!(
                "built `{}` but found no cdylib under {}",
                plan.package,
                plan.target_dir.join("debug").display()
            )
        })?;

    let dir = Path::new(".jedem");
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create .jedem: {e}"))?;
    let placed = dir.join(format!("{module}.{extension}"));
    std::fs::copy(&built, &placed)
        .map_err(|e| format!("could not place {}: {e}", placed.display()))?;
    Ok(placed)
}

pub fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}
