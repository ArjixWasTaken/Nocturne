use std::{path::Path, process::Command};

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let ui_out = Path::new(&out).join("ui");
    let blps: Vec<_> = walk("resources/ui");
    println!("cargo:rerun-if-changed=resources");

    let status = Command::new("blueprint-compiler")
        .args(["batch-compile", ui_out.to_str().unwrap(), "resources/ui"])
        .args(&blps)
        .status();
    println!("cargo:rerun-if-env-changed=NOCTURNE_PLACEHOLDER_UI");
    if !matches!(status, Ok(s) if s.success()) {
        // Opt-in escape hatch for `cargo check` on hosts whose libadwaita is too old for the templates.
        if std::env::var_os("NOCTURNE_PLACEHOLDER_UI").is_none() {
            panic!("blueprint-compiler failed (set NOCTURNE_PLACEHOLDER_UI=1 to build with empty templates)");
        }
        println!("cargo:warning=blueprint-compiler failed; UI templates will be empty placeholders");
        for blp in &blps {
            let rel = Path::new(blp).strip_prefix("resources/ui").unwrap().with_extension("ui");
            let dest = ui_out.join(rel);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(dest, "<interface/>").unwrap();
        }
    }

    glib_build_tools::compile_resources(
        &[out.as_str(), "resources"],
        "resources/nocturne.gresource.xml",
        "nocturne.gresource",
    );
}

fn walk(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(path.to_str().unwrap()));
        } else if path.extension().is_some_and(|e| e == "blp") {
            files.push(path.to_string_lossy().into_owned());
        }
    }
    files
}
