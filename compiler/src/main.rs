use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [dir] => run(dir, None),
        [dir, out] => run(dir, Some(std::path::PathBuf::from(out))),
        _ => {
            eprintln!("usage: html2gpui <root-dir> [output.rs]");
            std::process::exit(2);
        }
    }
}

fn run(dir: &str, out: Option<std::path::PathBuf>) {
    match html2gpui::compile_dir(Path::new(dir)) {
        Ok(compiled) => {
            for w in &compiled.warnings {
                eprintln!("warning: {w}");
            }
            match out {
                Some(p) => std::fs::write(p, compiled.code).unwrap(),
                None => print!("{}", compiled.code),
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
