//! A Zed-compatible JSON theme system.
//!
//! Theme files are the exact `assets/themes/*.json` files from
//! zed-industries/zed (One / Ayu / Gruvbox families), embedded via rust-embed
//! and parsed at first use. Color keys follow Zed's flat dotted schema
//! ("element.hover", "panel.background", ...).
//!
//! Token extraction lives in [`colors`]; this module owns parsing and the
//! theme registry.

mod colors;

pub use colors::Colors;

use std::sync::OnceLock;

use serde_json::{json, Value};

use self::colors::{parse_hex, FALLBACKS, KEY_MAP};

/// Embedded theme families to load, in menu order. GitHub is the default.
const THEME_FILES: &[&str] = &["themes/github.json", "themes/ayu.json", "themes/gruvbox.json"];

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    /// "dark" or "light", straight from the theme file.
    pub appearance: String,
    pub colors: Colors,
    /// JSON in gpui-component's `HighlightTheme` format (Zed-compatible):
    /// editor.* colors + the verbatim `syntax` token table.
    hl_json: String,
}

fn parse_family(json: &str, out: &mut Vec<Theme>) {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return;
    };
    let Some(themes) = v.get("themes").and_then(Value::as_array) else {
        return;
    };
    for t in themes {
        let Some(name) = t.get("name").and_then(Value::as_str) else {
            continue;
        };
        let appearance = t
            .get("appearance")
            .and_then(Value::as_str)
            .unwrap_or("dark")
            .to_string();
        // `style` is a flat object of dotted keys (+ syntax/players arrays we skip).
        let Some(style) = t.get("style").and_then(Value::as_object) else {
            continue;
        };
        let mut colors = Colors::all_missing();
        for (field, key) in KEY_MAP {
            if let Some(hex_str) = style.get(*key).and_then(Value::as_str) {
                if let Some(hex) = parse_hex(hex_str) {
                    if let Some(slot) = colors.get_mut(field) {
                        *slot = hex;
                    }
                }
            }
        }
        for (field, hex) in FALLBACKS {
            if let Some(slot) = colors.get_mut(field) {
                if *slot == Colors::MISSING {
                    *slot = *hex;
                }
            }
        }
        out.push(Theme {
            name: name.to_string(),
            appearance,
            hl_json: build_highlight_theme(name, t),
            colors,
        });
    }
}

/// Assemble the JSON consumed by `gpui_component::highlighter::HighlightTheme`,
/// whose schema is explicitly compatible with Zed's theme files. We forward
/// the editor chrome colors and the entire `syntax` table untouched, so the
/// tree-sitter captures are painted with exactly Zed's palette.
fn build_highlight_theme(name: &str, theme_obj: &Value) -> String {
    let style = theme_obj.get("style").cloned().unwrap_or(json!({}));
    let hl = json!({
        "name": name,
        "appearance": theme_obj.get("appearance").cloned().unwrap_or(json!("dark")),
        "style": {
            "editor.background": style.get("editor.background"),
            "editor.foreground": style.get("editor.foreground"),
            "editor.active_line.background": style.get("editor.active_line.background"),
            "editor.line_number": style.get("editor.line_number"),
            "editor.active_line_number": style.get("editor.active_line_number"),
            "error": style.get("text").or(style.get("editor.foreground")),
            "error.background": style.get("error.background").or(style.get("elevated_surface.background")).or(style.get("panel.background")),
            "error.border": style.get("error.border").or(style.get("border.variant")).or(style.get("border")),
            "warning": style.get("text").or(style.get("editor.foreground")),
            "warning.background": style.get("warning.background").or(style.get("elevated_surface.background")).or(style.get("panel.background")),
            "warning.border": style.get("warning.border").or(style.get("border.variant")).or(style.get("border")),
            "info": style.get("text").or(style.get("editor.foreground")),
            "info.background": style.get("info.background").or(style.get("elevated_surface.background")).or(style.get("panel.background")),
            "info.border": style.get("info.border").or(style.get("border.variant")).or(style.get("border")),
            "hint": style.get("text").or(style.get("editor.foreground")),
            "hint.background": style.get("hint.background").or(style.get("elevated_surface.background")).or(style.get("panel.background")),
            "hint.border": style.get("hint.border").or(style.get("border.variant")),
            "syntax": style.get("syntax"),
        },
    });
    hl.to_string()
}

impl Theme {
    /// Deserialize into the widget library's highlighter theme.
    pub fn highlight_theme(&self) -> gpui_component::highlighter::HighlightTheme {
        serde_json::from_str(&self.hl_json).unwrap_or_else(|_| {
            // Fall back to the library default if upstream JSON shifts.
            (*gpui_component::highlighter::HighlightTheme::default_dark()).clone()
        })
    }
}

/// All embedded themes, parsed once on first use.
pub fn all() -> &'static [Theme] {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| {
        let mut out = Vec::new();
        for file in THEME_FILES {
            if let Some(data) = crate::assets::AppAssets::get(file) {
                parse_family(std::str::from_utf8(&data.data).unwrap_or(""), &mut out);
            }
        }
        out
    })
}

/// Index of the startup theme ("GitHub Dark").
pub fn default_index() -> usize {
    all()
        .iter()
        .position(|t| t.name == "GitHub Dark")
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_themes() {
        let themes = all();
        assert_eq!(themes.len(), 18, "9 GitHub + 3 Ayu + 6 Gruvbox");
        assert!(themes.iter().any(|t| t.name == "GitHub Dark"));
        assert!(themes.iter().any(|t| t.name == "Ayu Mirage"));
        assert!(themes.iter().any(|t| t.name == "Gruvbox Light"));
        // One family was removed; GitHub is the default now.
        assert!(!themes.iter().any(|t| t.name == "One Dark"));
        assert_eq!(all()[default_index()].name, "GitHub Dark");
    }

    #[test]
    fn github_dark_values_match_source_file() {
        let gd = &all()[default_index()];
        assert_eq!(gd.appearance, "dark");
        assert_eq!(gd.colors.background, 0x0d1117ff);
        assert_eq!(gd.colors.panel, 0x010409ff);
        assert_eq!(gd.colors.editor_fg, 0xf0f6fcff);
    }

    #[test]
    fn every_theme_has_every_token() {
        for t in all() {
            assert!(
                t.colors.is_complete(),
                "theme '{}' is missing tokens",
                t.name
            );
        }
    }

    #[test]
    fn light_and_dark_appearances_exist_for_widget_sync() {
        assert!(all().iter().any(|t| t.appearance == "light"));
        assert!(all().iter().any(|t| t.appearance == "dark"));
    }

    #[test]
    fn highlighter_uses_zed_syntax_palette() {
        use gpui::{rgba, Hsla};
        let od = &all()[default_index()];
        let hl = od.highlight_theme();
        // GitHub Dark comment token is #9198a1 in the source JSON.
        let comment = hl.style.syntax.style("comment").expect("comment style");
        assert_eq!(comment.color, Some(Hsla::from(rgba(0x9198a1ff))));
        let function = hl.style.syntax.style("function").expect("function style");
        assert_eq!(function.color, Some(Hsla::from(rgba(0xd2a8ffff))));
        assert!(hl.style.syntax.style("keyword").is_some());
        assert!(hl.style.syntax.style("comment.doc").is_some());
    }
}
