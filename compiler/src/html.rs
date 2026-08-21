use ego_tree::NodeRef;
use scraper::{Html, Node};
use crate::css::{parse_css, parse_decls, tag_defaults};
use crate::template::split_template;
use crate::types::{Child, Ctx, IrElem, Result};
use crate::utils::{escape, trim_braces};

const TRANSPARENT: [&str; 3] = ["html", "head", "body"];
const SKIP_TAGS: [&str; 5] = ["style", "script", "title", "link", "meta"];

pub fn rewrite_component_tags(src: &str) -> String {
    static OPEN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static CLOSE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let open = OPEN.get_or_init(|| {
        regex::Regex::new(r#"<\s*([A-Z][A-Za-z0-9_]*)((?:[^>"']|"[^"]*"|'[^']*')*?)(\s*/)?\s*>"#)
            .unwrap()
    });
    let close = CLOSE
        .get_or_init(|| regex::Regex::new(r"</\s*[A-Z][A-Za-z0-9_]*\s*>").unwrap());

    let s = open.replace_all(src, |caps: &regex::Captures| {
        let name = &caps[1];
        let attrs = caps[2].trim();
        if caps.get(3).is_some() {
            if attrs.is_empty() {
                format!(r#"<comp data-comp="{name}"></comp>"#)
            } else {
                format!(r#"<comp data-comp="{name}" {attrs}></comp>"#)
            }
        } else if attrs.is_empty() {
            format!(r#"<comp data-comp="{name}">"#)
        } else {
            format!(r#"<comp data-comp="{name}" {attrs}>"#)
        }
    });
    close.replace_all(&s, "</comp>").to_string()
}

pub fn strip_uses(raw: &str) -> (String, Vec<(String, String)>) {
    let mut uses = Vec::new();
    let mut cleaned = String::new();
    let mut seen_tag = false;
    for line in raw.lines() {
        let t = line.trim_start();
        if !seen_tag && t.starts_with("@use") {
            let parts: Vec<&str> = t.split_whitespace().collect();
            if parts.len() >= 4 && parts[2] == "from" {
                let alias = parts[1].to_string();
                let import_path = parts[3..].join(" ");
                uses.push((alias, import_path));
                continue;
            }
        }
        if t.contains('<') {
            seen_tag = true;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    (cleaned, uses)
}

pub fn compile_component_ir(src: &str, ctx: &Ctx, warnings: &mut Vec<String>) -> Result<(IrElem, String)> {
    let doc = Html::parse_document(src);
    let css = collect_style_text(doc.tree.root(), String::new());
    let (tags, classes) = parse_css(&css, warnings);

    let mut merged_tags = ctx.tag_styles.clone();
    merged_tags.extend(tags);
    let mut merged_classes = ctx.class_styles.clone();
    merged_classes.extend(classes);

    let ctx = Ctx {
        comps: ctx.comps.clone(),
        tag_styles: merged_tags,
        class_styles: merged_classes,
    };
    let mut children = Vec::new();
    collect_children(doc.tree.root().children(), &ctx, warnings, &mut children)?;
    let script = collect_script_text(doc.tree.root());
    Ok((
        IrElem {
            tag: "root".into(),
            decls: Vec::new(),
            events: Vec::new(),
            cond: None,
            src: None,
            children,
        },
        script,
    ))
}

pub fn collect_script_text(node: NodeRef<'_, Node>) -> String {
    let mut acc = String::new();
    match node.value() {
        Node::Element(el) if el.name() == "script" => {
            for child in node.children() {
                if let Node::Text(t) = child.value() {
                    acc.push_str(t);
                    acc.push('\n');
                }
            }
        }
        _ => {
            for child in node.children() {
                acc.push_str(&collect_script_text(child));
            }
        }
    }
    acc
}

pub fn collect_style_text(node: NodeRef<'_, Node>, mut acc: String) -> String {
    match node.value() {
        Node::Element(el) if el.name() == "style" => {
            for child in node.children() {
                if let Node::Text(t) = child.value() {
                    acc.push_str(t.trim());
                    acc.push('\n');
                }
            }
            acc
        }
        _ => {
            for child in node.children() {
                acc = collect_style_text(child, acc);
            }
            acc
        }
    }
}

pub fn collect_children<'a, I>(nodes: I, ctx: &Ctx, warnings: &mut Vec<String>, out: &mut Vec<Child>) -> Result<()>
where
    I: Iterator<Item = NodeRef<'a, Node>>,
{
    for node in nodes {
        match node.value() {
            Node::Document
            | Node::Fragment
            | Node::Doctype(_)
            | Node::Comment(_)
            | Node::ProcessingInstruction(_) => {
                collect_children(node.children(), ctx, warnings, out)?;
            }
            Node::Text(t) => {
                let text: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    match split_template(&text) {
                        Some(parts) => out.push(Child::Tpl(parts)),
                        None => out.push(Child::Text(escape(&text))),
                    }
                }
            }
            Node::Element(el) => {
                let name = el.name();
                if TRANSPARENT.contains(&name) {
                    collect_children(node.children(), ctx, warnings, out)?;
                } else if SKIP_TAGS.contains(&name) {
                    continue;
                } else if name == "comp" {
                    let cname = el
                        .attr("data-comp")
                        .ok_or("internal error: comp element missing data-comp")?;
                    let f = ctx.comps.get(cname).ok_or_else(|| {
                        format!("unknown component <{cname}> (no {cname}.html found)")
                    })?;
                    let mut props = Vec::new();
                    for (k, v) in el.attrs() {
                        if k != "data-comp" {
                            props.push((k.to_string(), trim_braces(v)));
                        }
                    }
                    out.push(Child::Comp(f.clone(), props));
                } else {
                    if let Some(child) = gen_element(node, el.name(), el, ctx, warnings)? {
                        out.push(child);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn gen_element(
    node: NodeRef<'_, Node>,
    name: &str,
    el: &scraper::node::Element,
    ctx: &Ctx,
    warnings: &mut Vec<String>,
) -> Result<Option<Child>> {
    let mut decls: Vec<(String, String)> = tag_defaults(name);
    if let Some(list) = ctx.tag_styles.get(name) {
        decls.extend(list.iter().cloned());
    }
    if let Some(cls) = el.attr("class") {
        for c in cls.split_whitespace() {
            if let Some(list) = ctx.class_styles.get(c) {
                decls.extend(list.iter().cloned());
            }
        }
    }
    if let Some(style) = el.attr("style") {
        decls.extend(parse_decls(style));
    }

    if decls
        .iter()
        .any(|(p, v)| p == "display" && v.eq_ignore_ascii_case("none"))
    {
        return Ok(None);
    }

    const EVENT_ATTRS: [&str; 3] = ["onclick", "onhover", "onsubmit"];
    let events: Vec<(String, String)> = EVENT_ATTRS
        .iter()
        .filter_map(|a| {
            el.attr(a).map(|v| {
                let v = trim_braces(v);
                (a.trim_start_matches("on").to_string(), v)
            })
        })
        .collect();

    let cond = el.attr("if")
        .or_else(|| el.attr("show"))
        .or_else(|| el.attr("v-if"))
        .map(trim_braces);

    let src = el.attr("src")
        .or_else(|| el.attr("path"))
        .map(|s| s.to_string());

    if let Some(w) = el.attr("width") {
        decls.push(("width".into(), w.into()));
    }
    if let Some(h) = el.attr("height") {
        decls.push(("height".into(), h.into()));
    }
    if let Some(c) = el.attr("color").or_else(|| el.attr("fill")) {
        decls.push(("color".into(), c.into()));
    }

    let mut children = Vec::new();
    collect_children(node.children(), ctx, warnings, &mut children)?;
    Ok(Some(Child::Div(IrElem {
        tag: name.to_string(),
        decls,
        events,
        cond,
        src,
        children,
    })))
}
