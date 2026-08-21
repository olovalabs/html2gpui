use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use gpui::{
    div, svg, prelude::*, px, rgb, size, AnyElement, App, Application, AssetSource, Bounds, Context, Div,
    FontWeight, SharedString, Styled, Window, WindowBounds, WindowOptions,
};

use html2gpui::{display, Env, IrChild, IrDoc, IrElem, TplPart};

#[cfg(not(debug_assertions))]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

struct RootView {
    doc: Option<IrDoc>,
    env: Env,
    error: Option<String>,
    scroll_y: f32,
    #[cfg(debug_assertions)]
    last_hash: u64,
    #[cfg(debug_assertions)]
    last_check: Instant,
}

impl RootView {
    fn load() -> Self {
        match compile() {
            Ok((doc, warnings)) => {
                for w in &warnings {
                    eprintln!("[HMR] warning: {w}");
                }
                let env = match html2gpui::env_init(&doc.script) {
                    Ok(env) => env,
                    Err(e) => {
                        eprintln!("[state] init error: {e}");
                        Env::new()
                    }
                };
                eprintln!("[HMR] loaded ({} vars, {} fns)", env.len(), doc.script.funcs.len());
                Self {
                    doc: Some(doc),
                    env,
                    error: None,
                    scroll_y: 0.0,
                    #[cfg(debug_assertions)]
                    last_hash: hash_root(&find_root()),
                    #[cfg(debug_assertions)]
                    last_check: Instant::now(),
                }
            }
            Err(e) => {
                eprintln!("[HMR] initial compile error: {e}");
                Self {
                    doc: None,
                    env: Env::new(),
                    error: Some(e),
                    scroll_y: 0.0,
                    #[cfg(debug_assertions)]
                    last_hash: hash_root(&find_root()),
                    #[cfg(debug_assertions)]
                    last_check: Instant::now(),
                }
            }
        }
    }

    /// Recompiles the UI, preserving live variable values across reloads.
    fn reload(&mut self) {
        let res = compile();
        match res {
            Ok((doc, warnings)) => {
                for w in &warnings {
                    eprintln!("[HMR] warning: {w}");
                }
                if let Err(e) = html2gpui::env_merge(&mut self.env, &doc.script) {
                    eprintln!("[state] merge error: {e}");
                }
                eprintln!("[HMR] reloaded");
                self.doc = Some(doc);
                self.error = None;
            }
            Err(e) => {
                eprintln!("[HMR] compile error: {e}");
                self.error = Some(e);
            }
        }
    }
}

#[cfg(debug_assertions)]
fn compile() -> html2gpui::Result<(IrDoc, Vec<String>)> {
    html2gpui::compile_tree(&find_root())
}

#[cfg(not(debug_assertions))]
fn compile() -> html2gpui::Result<(IrDoc, Vec<String>)> {
    let files: Vec<(String, String)> = EMBEDDED_SOURCES
        .iter()
        .map(|(s, src)| (s.to_string(), src.to_string()))
        .collect();
    html2gpui::compile_sources(&files)
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let win_size = window.viewport_size();
        let win_h: f32 = win_size.height.into();
        let viewport_h = (win_h - 56.0).max(100.0);
        // Poll for file changes (~3x/sec) in dev builds.
        #[cfg(debug_assertions)]
        if self.last_check.elapsed() > Duration::from_millis(350) {
            self.last_check = Instant::now();
            if hash_root(&find_root()) != self.last_hash {
                self.last_hash = hash_root(&find_root());
                self.reload();
            }
            window.request_animation_frame();
        }

        let mut root = div().size_full().bg(rgb(0x1e1e2e)).text_color(rgb(0xcdd6f4));
        root = root.bg(rgb(0x09090b)).text_color(rgb(0xf4f4f5));

        if let Some(err) = &self.error {
            root = root.flex().flex_col().gap(px(8.0)).p(px(16.0)).child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0xff5555))
                    .child(SharedString::from(format!("Compile error: {err}"))),
            );
        }

        // Render the last good UI even while an error banner is shown.
        if let Some(doc) = self.doc.clone() {
            let env = self.env.clone();
            root = root.child(render_doc(&doc, &env, viewport_h, self.scroll_y, cx));
        } else if self.error.is_none() {
            root = root
                .justify_center()
                .items_center()
                .text_color(rgb(0x888888))
                .child(SharedString::from("Loading..."));
        }

        root
    }
}

fn render_doc(doc: &IrDoc, env: &Env, viewport_h: f32, scroll_y: f32, cx: &mut Context<RootView>) -> AnyElement {
    match doc.comps.get(&doc.entry) {
        Some(entry) => render_elem(entry, env, &doc.script, &doc.comps, "0", viewport_h, scroll_y, cx).unwrap_or_else(|| div().into_any_element()),
        None => div()
            .child(SharedString::from("missing entry app.html"))
            .into_any_element(),
    }
}

fn render_elem(
    elem: &IrElem,
    env: &Env,
    script: &html2gpui::IrScript,
    comps: &BTreeMap<String, IrElem>,
    path: &str,
    viewport_h: f32,
    scroll_y: f32,
    cx: &mut Context<RootView>,
) -> Option<AnyElement> {
    if let Some(cond_src) = &elem.cond {
        if !html2gpui::is_truthy_expr(env, script, cond_src) {
            return None;
        }
    }

    if (elem.tag == "svg" || elem.tag == "img") && elem.src.is_some() {
        let path_str = elem.src.as_ref().unwrap();
        let mut s = svg().path(SharedString::from(path_str.clone()));
        s = apply_svg_styles(s, &elem.decls);
        return Some(s.into_any_element());
    }

    let mut d = div();
    if path == "0" {
        d = d.size_full().flex().flex_row();
    }
    let d = apply_styles(d, &elem.decls);
    // `.id()` returns Stateful<Div>, which implements InteractiveElement
    // (needed for on_click / on_hover). Let the type be inferred.
    let mut d = d.id(SharedString::from(path.to_string()));

    let is_scrollable = elem.decls.iter().any(|(p, v)| {
        (p == "overflow" || p == "overflow-y") && (v == "auto" || v == "scroll")
    });
    let content_h = if is_scrollable {
        estimate_view_height(elem, env, script, comps).max(viewport_h)
    } else {
        0.0
    };
    let max_scroll = if is_scrollable && content_h > viewport_h {
        content_h - viewport_h + 30.0
    } else {
        0.0
    };

    if is_scrollable {
        d = d
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(move |v, event: &gpui::ScrollWheelEvent, _, cx| {
                if max_scroll <= 0.0 {
                    v.scroll_y = 0.0;
                    return;
                }
                let delta = match event.delta {
                    gpui::ScrollDelta::Lines(lines) => lines.y * 35.0,
                    gpui::ScrollDelta::Pixels(pixels) => {
                        let f: f32 = pixels.y.into();
                        f
                    }
                };
                v.scroll_y = (v.scroll_y + delta).min(0.0).max(-max_scroll);
                cx.notify();
            }));
    }

    // Attach event handlers: onclick="fn()" / onhover="fn()"
    for (ev, src) in &elem.events {
        let src = src.clone();
        match ev.as_str() {
            "click" => {
                d = d
                    .on_click(cx.listener(move |v, _, _, cx| run_action(v, &src, cx)));
            }
            "hover" => {
                d = d
                    .on_hover(cx.listener(move |v, hovered: &bool, _, cx| {
                        if *hovered {
                            run_action(v, &src, cx);
                        }
                    }));
            }
            _ => {}
        }
    }
    if is_scrollable {
        let mut inner = div().flex().flex_col().w_full().mt(px(scroll_y));
        for (i, child) in elem.children.iter().enumerate() {
            let child_path = format!("{path}/{i}");
            match child {
                IrChild::Text(t) => inner = inner.child(SharedString::from(unescape_text(t))),
                IrChild::Tpl(parts) => {
                    let s = render_template(parts, env);
                    inner = inner.child(SharedString::from(s));
                }
                IrChild::Comp(name, props) => match comps.get(name) {
                    Some(comp) => {
                        let mut child_env = env.clone();
                        for (k, expr_str) in props {
                        let val = match html2gpui::eval_expr_str(env, script, expr_str) {
                            Ok(v) => v,
                            Err(_) => {
                                let trimmed = expr_str.trim_matches('"').trim_matches('\'');
                                html2gpui::Value::Str(trimmed.to_string())
                            }
                        };
                            child_env.insert(k.clone(), val);
                        }
                        if let Some(el) = render_elem(comp, &child_env, script, comps, &child_path, viewport_h, scroll_y, cx) {
                            inner = inner.child(el);
                        }
                    }
                    None => inner = inner.child(SharedString::from(format!("[missing {name}]"))),
                },
                IrChild::Div(e) => {
                    if let Some(el) = render_elem(e, env, script, comps, &child_path, viewport_h, scroll_y, cx) {
                        inner = inner.child(el);
                    }
                }
            };
        }
        d = d.child(inner);
        if max_scroll > 0.0 {
            let scroll_ratio = (-scroll_y / max_scroll).clamp(0.0, 1.0);
            let thumb_h = ((viewport_h / content_h) * viewport_h).max(36.0).min(viewport_h - 20.0);
            let thumb_top = scroll_ratio * (viewport_h - thumb_h);
            let scrollbar = div()
                .absolute()
                .top(px(thumb_top))
                .right(px(4.0))
                .w(px(5.0))
                .h(px(thumb_h))
                .rounded(px(3.0))
                .bg(rgb(0x3f3f46));
            d = d.child(scrollbar);
        }
    } else {
        for (i, child) in elem.children.iter().enumerate() {
            let child_path = format!("{path}/{i}");
            match child {
                IrChild::Text(t) => d = d.child(SharedString::from(unescape_text(t))),
                IrChild::Tpl(parts) => {
                    let s = render_template(parts, env);
                    d = d.child(SharedString::from(s));
                }
                IrChild::Comp(name, props) => match comps.get(name) {
                    Some(comp) => {
                        let mut child_env = env.clone();
                        for (k, expr_str) in props {
                        let val = match html2gpui::eval_expr_str(env, script, expr_str) {
                            Ok(v) => v,
                            Err(_) => {
                                let trimmed = expr_str.trim_matches('"').trim_matches('\'');
                                html2gpui::Value::Str(trimmed.to_string())
                            }
                        };
                            child_env.insert(k.clone(), val);
                        }
                        if let Some(el) = render_elem(comp, &child_env, script, comps, &child_path, viewport_h, scroll_y, cx) {
                            d = d.child(el);
                        }
                    }
                    None => d = d.child(SharedString::from(format!("[missing {name}]"))),
                },
                IrChild::Div(e) => {
                    if let Some(el) = render_elem(e, env, script, comps, &child_path, viewport_h, scroll_y, cx) {
                        d = d.child(el);
                    }
                }
            };
        }
    }
    Some(d.into_any_element())
}

fn run_action(v: &mut RootView, src: &str, cx: &mut Context<RootView>) {
    let Some(doc) = &v.doc else { return };
    let script = doc.script.clone();
    if src.contains("current_page") {
        v.scroll_y = 0.0;
    }
    if let Err(e) = html2gpui::invoke(&mut v.env, &script, src) {
        eprintln!("[state] `{src}`: {e}");
    }
    cx.notify();
}

fn render_template(parts: &[TplPart], env: &Env) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            TplPart::Lit(s) => out.push_str(s),
            TplPart::Var(name) => match env.get(name) {
                Some(v) => out.push_str(&display(v)),
                None => out.push_str(&format!("?{name}")),
            },
        }
    }
    out
}

fn apply_styles(mut d: Div, decls: &[(String, String)]) -> Div {
    for (p, v) in decls {
        d = match p.as_str() {
            "display" => {
                if v.eq_ignore_ascii_case("flex") {
                    d.flex()
                } else {
                    d
                }
            }
            "flex" => {
                if v.trim() == "1" || v.starts_with("1 ") {
                    d.flex_1()
                } else {
                    d
                }
            }
            "flex-direction" => match v.as_str() {
                "row" => d.flex_row(),
                "column" => d.flex_col(),
                "row-reverse" => d.flex_row_reverse(),
                "column-reverse" => d.flex_col_reverse(),
                _ => d,
            },
            "justify-content" => match v.as_str() {
                "center" => d.justify_center(),
                "flex-start" => d.justify_start(),
                "flex-end" => d.justify_end(),
                "space-between" => d.justify_between(),
                "space-around" => d.justify_around(),
                "space-evenly" => d.justify_between(),
                _ => d,
            },
            "align-items" => match v.as_str() {
                "center" => d.items_center(),
                "flex-start" => d.items_start(),
                "flex-end" => d.items_end(),
                _ => d,
            },
            "gap" => {
                if let Some(n) = parse_px(v) {
                    d.gap(px(n))
                } else {
                    d
                }
            }
            "background-color" | "background" => {
                if let Some(c) = parse_color(v) {
                    d.bg(rgb(c))
                } else {
                    d
                }
            }
            "color" => {
                if let Some(c) = parse_color(v) {
                    d.text_color(rgb(c))
                } else {
                    d
                }
            }
            "font-size" => {
                if let Some(n) = parse_px(v) {
                    d.text_size(px(n))
                } else {
                    d
                }
            }
            "font-weight" => {
                let bold = v.eq_ignore_ascii_case("bold")
                    || v.eq_ignore_ascii_case("bolder")
                    || v.parse::<u32>().map(|n| n >= 600).unwrap_or(false);
                if bold {
                    d.font_weight(FontWeight::BOLD)
                } else {
                    d
                }
            }
            "width" => {
                if v.trim() == "100%" {
                    d.w_full()
                } else if let Some(n) = parse_px(v) {
                    d.w(px(n))
                } else if let Some(p) = parse_percent(v) {
                    d.w(gpui::relative(p))
                } else {
                    d
                }
            }
            "height" => {
                if v.trim() == "100%" {
                    d.h_full()
                } else if let Some(n) = parse_px(v) {
                    d.h(px(n))
                } else if let Some(p) = parse_percent(v) {
                    d.h(gpui::relative(p))
                } else {
                    d
                }
            }
            "min-height" => {
                if v.trim() == "100%" {
                    d.h_full()
                } else if let Some(n) = parse_px(v) {
                    d.min_h(px(n))
                } else {
                    d
                }
            }
            "min-width" => {
                if v.trim() == "100%" {
                    d.w_full()
                } else if let Some(n) = parse_px(v) {
                    d.min_w(px(n))
                } else {
                    d
                }
            }
            "overflow" | "overflow-y" => {
                if v.eq_ignore_ascii_case("hidden") {
                    d.overflow_hidden()
                } else {
                    d
                }
            }
            "padding" => {
                let vals: Vec<&str> = v.split_whitespace().collect();
                match vals.as_slice() {
                    [a] => {
                        if let Some(n) = parse_px(a) {
                            d.pt(px(n)).pb(px(n)).pl(px(n)).pr(px(n))
                        } else {
                            d
                        }
                    }
                    [a, b] => {
                        if let Some(n) = parse_px(a) {
                            d = d.pt(px(n)).pb(px(n));
                        }
                        if let Some(n) = parse_px(b) {
                            d = d.pl(px(n)).pr(px(n));
                        }
                        d
                    }
                    _ => d,
                }
            }
            "margin" => {
                let vals: Vec<&str> = v.split_whitespace().collect();
                match vals.as_slice() {
                    [a] => {
                        if let Some(n) = parse_px(a) {
                            d.mt(px(n)).mb(px(n)).ml(px(n)).mr(px(n))
                        } else {
                            d
                        }
                    }
                    [a, b] => {
                        if let Some(n) = parse_px(a) {
                            d = d.mt(px(n)).mb(px(n));
                        }
                        if let Some(n) = parse_px(b) {
                            d = d.ml(px(n)).mr(px(n));
                        }
                        d
                    }
                    _ => d,
                }
            }
            "border-radius" => {
                if let Some(n) = parse_px(v) {
                    d.rounded(px(n))
                } else {
                    d
                }
            }
            "border-color" => {
                if let Some(c) = parse_color(v) {
                    d.border_color(rgb(c))
                } else {
                    d
                }
            }
            "border-width" => {
                if let Some(n) = parse_px(v) {
                    d.border(px(n))
                } else {
                    d
                }
            }
            "border" => {
                for part in v.split_whitespace() {
                    if let Some(n) = parse_px(part) {
                        d = d.border(px(n));
                    } else if let Some(c) = parse_color(part) {
                        d = d.border_color(rgb(c));
                    }
                }
                d
            }
            "opacity" => {
                if let Some(f) = v.parse::<f32>().ok().filter(|f| (0.0..=1.0).contains(f)) {
                    d.opacity(f)
                } else {
                    d
                }
            }
            _ => d,
        };
    }
    d
}

fn apply_svg_styles(mut s: gpui::Svg, decls: &[(String, String)]) -> gpui::Svg {
    let _h = gpui::ScrollHandle::new();
    for (p, v) in decls {
        s = match p.as_str() {
            "width" => {
                if let Some(n) = parse_px(v) {
                    s.w(px(n))
                } else {
                    s
                }
            }
            "height" => {
                if let Some(n) = parse_px(v) {
                    s.h(px(n))
                } else {
                    s
                }
            }
            "color" | "fill" => {
                if let Some(c) = parse_color(v) {
                    s.text_color(rgb(c))
                } else {
                    s
                }
            }
            _ => s,
        };
    }
    s
}

fn estimate_view_height(
    elem: &IrElem,
    env: &Env,
    script: &html2gpui::IrScript,
    comps: &BTreeMap<String, IrElem>,
) -> f32 {
    let is_row = elem.decls.iter().any(|(k, v)| k == "flex-direction" && v.contains("row"));
    let mut total_col = 0.0f32;
    let mut max_row_h = 0.0f32;

    for child in &elem.children {
        match child {
            IrChild::Text(_) | IrChild::Tpl(_) => {
                if is_row {
                    max_row_h = max_row_h.max(22.0);
                } else {
                    total_col += 22.0;
                }
            }
            IrChild::Comp(name, _) => {
                if let Some(comp) = comps.get(name) {
                    let h = estimate_view_height(comp, env, script, comps);
                    if is_row {
                        max_row_h = max_row_h.max(h);
                    } else {
                        total_col += h;
                    }
                }
            }
            IrChild::Div(e) => {
                if let Some(cond) = &e.cond {
                    if !html2gpui::is_truthy_expr(env, script, cond) {
                        continue;
                    }
                }
                let explicit_h = e
                    .decls
                    .iter()
                    .find(|(k, _)| k == "height")
                    .and_then(|(_, v)| parse_px(v));
                let item_h = if let Some(h) = explicit_h {
                    h
                } else {
                    let child_h = estimate_view_height(e, env, script, comps);
                    let pad = e
                        .decls
                        .iter()
                        .find(|(k, _)| k == "padding")
                        .and_then(|(_, v)| parse_px(v))
                        .unwrap_or(0.0);
                    let gap = e
                        .decls
                        .iter()
                        .find(|(k, _)| k == "gap")
                        .and_then(|(_, v)| parse_px(v))
                        .unwrap_or(0.0);
                    child_h.max(26.0) + pad * 2.0 + gap
                };
                if is_row {
                    max_row_h = max_row_h.max(item_h);
                } else {
                    let item_gap = elem.decls.iter().find(|(k, _)| k == "gap").and_then(|(_, v)| parse_px(v)).unwrap_or(8.0);
                    total_col += item_h + item_gap;
                }
            }
        }
    }
    let pad_self = elem.decls.iter().find(|(k, _)| k == "padding").and_then(|(_, v)| parse_px(v)).unwrap_or(0.0);
    if is_row {
        max_row_h + pad_self * 2.0
    } else {
        total_col + pad_self * 2.0
    }
}

fn parse_px(v: &str) -> Option<f32> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("px") {
        n.trim().parse().ok()
    } else if let Ok(n) = v.parse::<f32>() {
        Some(n)
    } else {
        None
    }
}

fn parse_percent(v: &str) -> Option<f32> {
    v.trim()
        .strip_suffix('%')
        .and_then(|n| n.trim().parse::<f32>().ok())
        .map(|n| n / 100.0)
}

fn parse_color(v: &str) -> Option<u32> {
    let v = v.trim();
    if let Some(hex) = v.strip_prefix('#') {
        let hex = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => hex.to_string(),
            _ => return None,
        };
        return u32::from_str_radix(&hex, 16).ok();
    }
    match v.to_ascii_lowercase().as_str() {
        "white" => Some(0xffffff),
        "black" => Some(0x000000),
        "red" => Some(0xff0000),
        "green" => Some(0x008000),
        "lime" => Some(0x00ff00),
        "blue" => Some(0x0000ff),
        "yellow" => Some(0xffff00),
        "orange" => Some(0xffa500),
        "purple" => Some(0x800080),
        "pink" => Some(0xffc0cb),
        "gray" | "grey" => Some(0x808080),
        _ => None,
    }
}

// ---------- shared helpers ----------

#[cfg(debug_assertions)]
fn find_root() -> std::path::PathBuf {
    let candidates = [
        std::path::PathBuf::from("root"),
        std::path::PathBuf::from("app/root"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|pp| pp.join("root")))
            .unwrap_or_default(),
        std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent()
                    .and_then(|pp| pp.parent())
                    .map(|pp| pp.join("root"))
            })
            .unwrap_or_default(),
        std::path::PathBuf::from("../root"),
    ];
    for p in candidates {
        if p.is_dir() && p.join("app.html").exists() {
            return p;
        }
    }
    std::path::PathBuf::from("root")
}

#[cfg(debug_assertions)]
fn hash_root(root: &std::path::Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    fn walk_hash(dir: &std::path::Path, h: &mut DefaultHasher) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                if p.is_dir() {
                    walk_hash(&p, h);
                } else if p.extension().is_some_and(|e| e == "html" || e == "css") {
                    p.file_name().unwrap_or_default().to_string_lossy().hash(h);
                    if let Ok(s) = std::fs::read_to_string(&p) {
                        s.hash(h);
                    }
                }
            }
        }
    }
    walk_hash(root, &mut h);
    h.finish()
}

/// Inverts the compiler's text escaping (codegen strings store escaped text).
fn unescape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct FileAssetSource {
    base: std::path::PathBuf,
}

impl AssetSource for FileAssetSource {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let clean_path = path.trim_start_matches("./").trim_start_matches('/');
        let candidates = [
            self.base.join(clean_path),
            std::path::PathBuf::from(clean_path),
            std::path::PathBuf::from("root").join(clean_path),
            std::path::PathBuf::from("root/icons").join(clean_path),
            std::path::PathBuf::from("root/assets").join(clean_path),
        ];
        for p in candidates {
            if let Ok(bytes) = std::fs::read(&p) {
                return Ok(Some(std::borrow::Cow::Owned(bytes)));
            }
        }
        Ok(None)
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(self.base.join(path)) {
            for entry in entries.flatten() {
                if let Some(s) = entry.path().to_str() {
                    list.push(SharedString::from(s.to_string()));
                }
            }
        }
        Ok(list)
    }
}

fn main() {
    let assets = FileAssetSource { base: std::path::PathBuf::from("root") };
    Application::new().with_assets(assets).run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(860.), px(580.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| RootView::load()),
        )
        .unwrap();
        cx.activate(true);
    });
}
