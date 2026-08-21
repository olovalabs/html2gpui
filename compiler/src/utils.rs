pub fn trim_braces(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('{') && v.ends_with('}') && v.len() >= 2 {
        v[1..v.len() - 1].trim().to_string()
    } else {
        v.to_string()
    }
}

pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}

pub fn pascal_case(stem: &str) -> String {
    stem.split(['-', '_', ' '])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub fn snake_case(stem: &str) -> String {
    let mut out = String::new();
    for (i, ch) in stem.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else if ch == '-' || ch == ' ' {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out
}
