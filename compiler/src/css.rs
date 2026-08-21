use std::collections::BTreeMap;

pub fn parse_css(
    css: &str,
    warnings: &mut Vec<String>,
) -> (
    BTreeMap<String, Vec<(String, String)>>,
    BTreeMap<String, Vec<(String, String)>>,
) {
    let mut clean_css = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        clean_css.push_str(&rest[..start]);
        if let Some(end) = rest[start + 2..].find("*/") {
            rest = &rest[start + 2 + end + 2..];
        } else {
            rest = "";
            break;
        }
    }
    clean_css.push_str(rest);
    let css = &clean_css;

    let mut tags = BTreeMap::new();
    let mut classes = BTreeMap::new();
    let mut i = 0usize;
    while let Some(rel) = css[i..].find('{') {
        let open = i + rel;
        let selector = css[i..open].trim();
        let Some(close_rel) = css[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let body = &css[open + 1..close];
        i = close + 1;

        for sel in selector.split(',').map(str::trim) {
            if sel.is_empty() {
                continue;
            }
            if let Some(cls) = sel.strip_prefix('.') {
                if !cls.contains(['.', ':', ' ', '>', '[', '#']) {
                    classes.insert(cls.to_string(), parse_decls(body));
                    continue;
                }
            } else if sel.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                tags.insert(sel.to_ascii_lowercase(), parse_decls(body));
                continue;
            }
            warnings.push(format!("unsupported css selector `{sel}` (only `tag {{ }}` and `.class {{ }}`)"));
        }
    }
    (tags, classes)
}

pub fn parse_decls(body: &str) -> Vec<(String, String)> {
    body.split(';')
        .filter_map(|d| {
            let d = d.trim();
            if d.is_empty() {
                return None;
            }
            let (p, v) = d.split_once(':')?;
            Some((p.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

pub fn tag_defaults(tag: &str) -> Vec<(String, String)> {
    match tag {
        "h1" => vec![("font-size".into(), "32px".into()), ("font-weight".into(), "bold".into())],
        "h2" => vec![("font-size".into(), "28px".into()), ("font-weight".into(), "bold".into())],
        "h3" => vec![("font-size".into(), "24px".into()), ("font-weight".into(), "bold".into())],
        "h4" => vec![("font-size".into(), "20px".into()), ("font-weight".into(), "bold".into())],
        "h5" => vec![("font-size".into(), "16px".into()), ("font-weight".into(), "bold".into())],
        "h6" => vec![("font-size".into(), "14px".into()), ("font-weight".into(), "bold".into())],
        "b" | "strong" => vec![("font-weight".into(), "bold".into())],
        _ => Vec::new(),
    }
}

pub fn decl_methods(p: &str, v: &str, warnings: &mut Vec<String>, tag: &str) -> Vec<String> {
    let mut warn = |msg: String| warnings.push(format!("<{tag}> {p}: {msg}"));
    match p {
        "display" => {
            if v.eq_ignore_ascii_case("flex") {
                vec!["flex()".into()]
            } else {
                Vec::new()
            }
        }
        "flex-direction" => match v.trim() {
            "row" => vec!["flex_row()".into()],
            "column" => vec!["flex_col()".into()],
            "row-reverse" => vec!["flex_row_reverse()".into()],
            "column-reverse" => vec!["flex_col_reverse()".into()],
            other => {
                warn(format!("unsupported value `{other}`"));
                Vec::new()
            }
        },
        "justify-content" => match v.trim() {
            "center" => vec!["justify_center()".into()],
            "flex-start" => vec!["justify_start()".into()],
            "flex-end" => vec!["justify_end()".into()],
            "space-between" => vec!["justify_between()".into()],
            "space-around" => vec!["justify_around()".into()],
            "space-evenly" => vec!["justify_between()".into()],
            other => {
                warn(format!("unsupported value `{other}`"));
                Vec::new()
            }
        },
        "align-items" => match v.trim() {
            "center" => vec!["items_center()".into()],
            "flex-start" => vec!["items_start()".into()],
            "flex-end" => vec!["items_end()".into()],
            other => {
                warn(format!("unsupported value `{other}`"));
                Vec::new()
            }
        },
        "gap" => len_expr(v).map(|l| vec![format!("gap({l})")]).unwrap_or_else(|| {
            warn(format!("unsupported value `{v}`"));
            Vec::new()
        }),
        "background-color" | "background" => color_expr(v)
            .map(|c| vec![format!("bg(rgb(0x{c:06x}))")])
            .unwrap_or_else(|| {
                warn(format!("unsupported color `{v}`"));
                Vec::new()
            }),
        "color" => color_expr(v)
            .map(|c| vec![format!("text_color(rgb(0x{c:06x}))")])
            .unwrap_or_else(|| {
                warn(format!("unsupported color `{v}`"));
                Vec::new()
            }),
        "font-size" => {
            if let Some(n) = parse_px_str(v) {
                vec![format!("text_size(px({}))", n)]
            } else {
                warn(format!("unsupported size `{v}` (use px)"));
                Vec::new()
            }
        }
        "font-weight" => {
            let bold = v.eq_ignore_ascii_case("bold")
                || v.eq_ignore_ascii_case("bolder")
                || v.parse::<u32>().map(|n| n >= 600).unwrap_or(false);
            if bold {
                vec!["font_weight(FontWeight::BOLD)".into()]
            } else {
                Vec::new()
            }
        }
        "width" => len_expr(v)
            .map(|l| {
                if l == "full" {
                    vec!["w_full()".into()]
                } else {
                    vec![format!("w({l})")]
                }
            })
            .unwrap_or_default(),
        "height" => len_expr(v)
            .map(|l| {
                if l == "full" {
                    vec!["h_full()".into()]
                } else {
                    vec![format!("h({l})")]
                }
            })
            .unwrap_or_default(),
        "padding" => box_expand(v, &["pt", "pb", "pl", "pr"]),
        "margin" => box_expand(v, &["mt", "mb", "ml", "mr"]),
        "border-radius" => len_expr(v)
            .map(|l| vec![format!("rounded({l})")])
            .unwrap_or_default(),
        "opacity" => v
            .parse::<f64>()
            .ok()
            .filter(|n| (0.0..=1.0).contains(n))
            .map(|n| vec![format!("opacity({})", n)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub fn box_expand(v: &str, sides: &[&str; 4]) -> Vec<String> {
    let vals: Vec<&str> = v.split_whitespace().collect();
    let (block, inline) = match vals.as_slice() {
        [a] => (*a, *a),
        [a, b] => (*a, *b),
        _ => return Vec::new(),
    };
    let (t, b, l, r) = (sides[0], sides[1], sides[2], sides[3]);
    let mut out = Vec::new();
    if let Some(e) = len_expr(block) {
        out.push(format!("{t}({e})"));
        out.push(format!("{b}({e})"));
    }
    if let Some(e) = len_expr(inline) {
        out.push(format!("{l}({e})"));
        out.push(format!("{r}({e})"));
    }
    out
}

pub fn len_expr(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("px") {
        let n: f64 = n.trim().parse().ok()?;
        Some(format!("px({})", num(n)))
    } else if let Some(n) = v.strip_suffix('%') {
        let n: f64 = n.trim().parse().ok()?;
        if (n - 100.0).abs() < 1e-9 {
            Some("full".into())
        } else {
            Some(format!("relative({})", n / 100.0))
        }
    } else {
        let n: f64 = v.parse().ok()?;
        Some(format!("px({})", num(n)))
    }
}

pub fn num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{n:.1}")
    } else {
        format!("{n}")
    }
}

pub fn parse_px_str(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("px") {
        n.trim().parse::<f64>().ok().map(num)
    } else if let Ok(n) = v.parse::<f64>() {
        Some(num(n))
    } else {
        None
    }
}

pub fn color_expr(v: &str) -> Option<u32> {
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
