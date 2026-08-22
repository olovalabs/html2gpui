use std::collections::BTreeMap;
use std::path::Path;
use crate::css::parse_css;
use crate::html::{compile_component_ir, rewrite_component_tags, strip_script_imports};
use crate::script::parse_script;
use crate::types::{Ctx, IrDoc, Result};
use crate::utils::{pascal_case, snake_case};

pub fn compile_tree(root_dir: &Path) -> Result<(IrDoc, Vec<String>)> {
    let mut files = Vec::new();
    collect_files_recursive(root_dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    compile_sources(&files)
}

pub fn collect_files_recursive(dir: &Path, files: &mut Vec<(String, String)>) -> Result<()> {
    collect_files_inner(dir, dir, files)
}

fn collect_files_inner(base: &Path, current: &Path, files: &mut Vec<(String, String)>) -> Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(current)
        .map_err(|e| format!("cannot read folder {}: {e}", current.display()))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_files_inner(base, &p, files)?;
        } else if p.extension().is_some_and(|e| e == "html" || e == "css") {
            let rel = p.strip_prefix(base).unwrap_or(&p);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let src = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            files.push((rel_str, src));
        }
    }
    Ok(())
}

pub fn compile_sources(files: &[(String, String)]) -> Result<(IrDoc, Vec<String>)> {
    if files.is_empty() {
        return Err("no .html or .css files provided".into());
    }

    let mut html_files = Vec::new();
    let mut css_files: BTreeMap<String, String> = BTreeMap::new();
    for (name, raw) in files {
        if name.ends_with(".css") {
            css_files.insert(name.clone(), raw.clone());
        } else {
            let stem = name.strip_suffix(".html").unwrap_or(name);
            html_files.push((stem.to_string(), raw.clone()));
        }
    }

    if !html_files.iter().any(|(stem, _)| stem == "app" || stem.ends_with("/app")) {
        return Err("missing entry component app.html".into());
    }

    let mut warnings = Vec::new();

    // First pass: pull import lines out of every component so we know which
    // css files are explicitly imported before deciding what stays global.
    struct Source {
        rel_path: String,
        file_stem: String,
        comp_id: String,
        pascal_name: String,
        src: String,
        uses: Vec<(String, String)>,
        css_paths: Vec<String>,
        local_tags: BTreeMap<String, Vec<(String, String)>>,
        local_classes: BTreeMap<String, Vec<(String, String)>>,
    }
    let mut sources: Vec<Source> = Vec::new();
    for (rel_path, raw) in &html_files {
        let (src, imports) = strip_script_imports(raw, &mut warnings);
        let src = rewrite_component_tags(&src);

        let mut local_tags = BTreeMap::new();
        let mut local_classes = BTreeMap::new();
        for css_path in &imports.css {
            let clean = normalize_path(css_path);
            let Some(css_src) = css_files.get(&clean) else {
                return Err(format!(
                    "import \"{css_path}\": no matching css file found in root/"
                ));
            };
            let (tags, classes) = parse_css(css_src, &mut warnings);
            merge_decls(&mut local_tags, tags);
            merge_decls(&mut local_classes, classes);
        }

        let file_stem = rel_path.split('/').last().unwrap_or(rel_path).to_string();
        let comp_id = snake_case(&file_stem);
        let pascal_name = pascal_case(&file_stem);
        sources.push(Source {
            rel_path: rel_path.clone(),
            file_stem,
            comp_id,
            pascal_name,
            src,
            uses: imports.comps,
            css_paths: imports.css.iter().map(|p| normalize_path(p)).collect(),
            local_tags,
            local_classes,
        });
    }

    // CSS that is explicitly imported is scoped to its importer; everything
    // else stays global.
    let imported_css: Vec<String> = sources
        .iter()
        .flat_map(|s| s.css_paths.iter().cloned())
        .collect();
    let global_css: String = css_files
        .iter()
        .filter(|(name, _)| !imported_css.contains(name))
        .map(|(_, src)| src.clone() + "\n")
        .collect();
    let (global_tags, global_classes) = parse_css(&global_css, &mut warnings);

    let mut comps = BTreeMap::new();
    let mut script_src = String::new();
    for s in &sources {
        let mut local_ctx = Ctx {
            comps: BTreeMap::new(),
            tag_styles: global_tags.clone(),
            class_styles: global_classes.clone(),
        };
        for other in &sources {
            local_ctx.comps.insert(other.pascal_name.clone(), other.comp_id.clone());
        }
        for (alias, path_str) in &s.uses {
            let clean = path_str
                .trim_matches('"')
                .trim_matches('\'') 
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches(".html")
                .replace('\\', "/");
            let target = sources
                .iter()
                .find(|src| src.rel_path == clean || src.file_stem == clean || src.file_stem == clean.split('/').last().unwrap_or(&clean))
                .or_else(|| sources.iter().find(|src| src.pascal_name == *alias || src.file_stem == snake_case(alias)));

            if let Some(matched) = target {
                local_ctx.comps.insert(alias.clone(), matched.comp_id.clone());
            } else {
                return Err(format!(
                    "import {alias} from \"{path_str}\": no matching component file found in root/"
                ));
            }
        }
        local_ctx.tag_styles.extend(s.local_tags.clone());
        local_ctx.class_styles.extend(s.local_classes.clone());
        let (elem, script) = compile_component_ir(&s.src, &local_ctx, &mut warnings)
            .map_err(|e| format!("{}.html: {e}", s.rel_path))?;
        script_src.push_str(&script);
        script_src.push('\n');
        comps.insert(s.comp_id.clone(), elem);
    }
    let script = parse_script(&script_src).map_err(|e| format!("script error: {e}"))?;
    Ok((
        IrDoc {
            entry: "app".to_string(),
            comps,
            script,
        },
        warnings,
    ))
}

fn normalize_path(p: &str) -> String {
    p.trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("./")
        .trim_start_matches('/')
        .replace('\\', "/")
}

fn merge_decls(
    into: &mut BTreeMap<String, Vec<(String, String)>>,
    from: BTreeMap<String, Vec<(String, String)>>,
) {
    for (k, v) in from {
        into.entry(k).or_insert(v);
    }
}
