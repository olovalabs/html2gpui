use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=root");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root_dir = Path::new(&manifest_dir)
        .parent()
        .expect("package dir has no parent")
        .join("root");

    match html2gpui::compile_dir(&root_dir) {
        Ok(compiled) => {
            for w in &compiled.warnings {
                println!("cargo:warning=html2gpui: {w}");
            }
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let mut generated = compiled.code;
            // Embed the raw component sources so release builds run the same
            // stateful runtime renderer as dev (no filesystem access needed).
            generated.push_str("\n/// (file stem, source text) pairs, sorted by stem\n");
            generated.push_str("#[allow(dead_code)]\n");
            generated.push_str("pub static EMBEDDED_SOURCES: &[(&str, &str)] = &[\n");
            fn collect_sources_recursive(dir: &Path, sources: &mut Vec<(String, String)>) {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            collect_sources_recursive(&p, sources);
                        } else if p.extension().is_some_and(|e| e == "html" || e == "css") {
                            if let (Some(stem), Some(ext)) = (p.file_stem(), p.extension()) {
                                let stem = stem.to_string_lossy().to_string();
                                let ext = ext.to_string_lossy().to_string();
                                let name = if ext == "css" { format!("{stem}.css") } else { stem };
                                if let Ok(src) = std::fs::read_to_string(&p) {
                                    sources.push((name, src));
                                }
                            }
                        }
                    }
                }
            }
            let mut sources = Vec::new();
            collect_sources_recursive(&root_dir, &mut sources);
            sources.sort();
            for (stem, src) in &sources {
                generated.push_str(&format!("    ({stem:?}, {src:?}),\n"));
            }
            generated.push_str("];\n");
            std::fs::write(Path::new(&out_dir).join("generated.rs"), generated).unwrap();
        }
        Err(e) => panic!("html2gpui error:\n{e}"),
    }
}
