use crate::types::TplPart;

pub fn split_template(text: &str) -> Option<Vec<TplPart>> {
    let mut parts = Vec::new();
    let mut rest = text;
    let mut found_var = false;

    while let Some(start) = rest.find('{') {
        let is_double = rest[start..].starts_with("{{");
        let (open_len, close_delim) = if is_double { (2, "}}") } else { (1, "}") };

        let after = &rest[start + open_len..];
        if let Some(end) = after.find(close_delim) {
            let name = after[..end].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                if start > 0 {
                    parts.push(TplPart::Lit(rest[..start].to_string()));
                }
                parts.push(TplPart::Var(name.to_string()));
                found_var = true;
                rest = &after[end + close_delim.len()..];
                continue;
            }
        }
        parts.push(TplPart::Lit(rest[..start + 1].to_string()));
        rest = &rest[start + 1..];
    }

    if !found_var {
        return None;
    }

    if !rest.is_empty() {
        parts.push(TplPart::Lit(rest.to_string()));
    }

    let mut merged = Vec::new();
    for p in parts {
        match p {
            TplPart::Lit(s) if !s.is_empty() => {
                if let Some(TplPart::Lit(prev)) = merged.last_mut() {
                    prev.push_str(&s);
                } else {
                    merged.push(TplPart::Lit(s));
                }
            }
            TplPart::Var(v) => merged.push(TplPart::Var(v)),
            _ => {}
        }
    }
    Some(merged)
}
