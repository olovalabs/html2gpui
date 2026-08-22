use crate::ast::Expr;
use crate::parser::Parser;
use crate::tokenizer::tokenize;
use crate::types::{Result, TplPart};

/// Splits text into literal and expression parts. Inside `{}` any JSX-style
/// expression is accepted: `{count}`, `{1 + 1}`, `{a > b ? x : y}`,
/// `{console.log()}`, string literals, arithmetic, comparisons, etc.
pub fn split_template(text: &str) -> Option<Vec<TplPart>> {
    let mut parts = Vec::new();
    let mut rest = text;
    let mut found_var = false;

    while let Some(start) = find_open_brace(rest) {
        let after = &rest[start + 1..];
        if let Some((expr_src, skip)) = scan_expr(after) {
            match parse_expr(&expr_src) {
                Ok(expr) => {
                    if start > 0 {
                        parts.push(TplPart::Lit(rest[..start].to_string()));
                    }
                    // Keep bare names as Var so state/props lookup stays fast.
                    if let Expr::Var(name) = &expr {
                        parts.push(TplPart::Var(name.clone()));
                    } else {
                        parts.push(TplPart::Expr(expr));
                    }
                    found_var = true;
                    rest = &after[skip..];
                    continue;
                }
                Err(_) => {
                    // Not a valid expression: treat `{` as literal text.
                }
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

    Some(merge_literals(parts))
}

fn parse_expr(src: &str) -> Result<Expr> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    p.expr()
}

fn merge_literals(parts: Vec<TplPart>) -> Vec<TplPart> {
    let mut merged: Vec<TplPart> = Vec::new();
    for p in parts {
        match p {
            TplPart::Lit(s) if !s.is_empty() => {
                if let Some(TplPart::Lit(prev)) = merged.last_mut() {
                    prev.push_str(&s);
                } else {
                    merged.push(TplPart::Lit(s));
                }
            }
            other => merged.push(other),
        }
    }
    merged
}

/// Finds the next `{` (the first byte of either `{expr}` or `{{expr}}`).
fn find_open_brace(s: &str) -> Option<usize> {
    s.as_bytes().iter().position(|&b| b == b'{')
}

/// Scans from just after `{` to its matching close, honoring strings and
/// brace nesting. Returns the expression source and the offset (within
/// `after`) just past the closing `}` / `}}`.
fn scan_expr(after: &str) -> Option<(String, usize)> {
    let is_double = after.starts_with('{');
    let skip = if is_double { 1 } else { 0 };
    let bytes = after.as_bytes();
    let mut depth = 1usize;
    let mut i = skip;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                in_str = None;
            }
        } else {
            match c {
                b'"' | b'\'' => in_str = Some(c),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        if is_double {
                            if after[i + 1..].starts_with('}') {
                                return Some((after[skip..i].to_string(), i + 2));
                            }
                            return None;
                        }
                        return Some((after[skip..i].to_string(), i + 1));
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}
