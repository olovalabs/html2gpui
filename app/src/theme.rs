//! A Zed-compatible JSON theme system.
//!
//! Theme files are the exact `assets/themes/*.json` files from
//! zed-industries/zed (One / Ayu / Gruvbox families), embedded via rust-embed
//! and parsed at first use. Color keys follow Zed's flat dotted schema
//! ("element.hover", "panel.background", ...).
//!
//! Only the subset of tokens this app actually renders is extracted; extend
//! [`Colors`], [`KEY_MAP`] and [`Colors::get_mut`] as the UI grows.

use std::sync::OnceLock;

use serde_json::Value;

/// The color tokens used by the app, named after Zed's theme keys.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub background: u32,
    pub surface: u32,
    pub elevated_surface: u32,
    pub element_bg: u32,
    pub element_hover: u32,
    pub element_active: u32,
    pub element_selected: u32,
    pub ghost_hover: u32,
    pub ghost_active: u32,
    pub border: u32,
    pub border_variant: u32,
    pub border_focused: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_accent: u32,
    pub icon: u32,
    pub icon_muted: u32,
    pub icon_accent: u32,
    pub title_bar: u32,
    pub status_bar: u32,
    pub panel: u32,
    pub toolbar: u32,
    pub tab_bar: u32,
    pub editor_bg: u32,
    pub editor_fg: u32,
    pub terminal_bg: u32,
    pub vc_added: u32,
    pub vc_modified: u32,
    pub vc_deleted: u32,
}

impl Colors {
    /// Sentinel for "key missing in theme file" so gaps are loud, not silent.
    const MISSING: u32 = 0xFF00FF;

    fn all_missing() -> Self {
        Self {
            background: Self::MISSING,
            surface: Self::MISSING,
            elevated_surface: Self::MISSING,
            element_bg: Self::MISSING,
            element_hover: Self::MISSING,
            element_active: Self::MISSING,
            element_selected: Self::MISSING,
            ghost_hover: Self::MISSING,
            ghost_active: Self::MISSING,
            border: Self::MISSING,
            border_variant: Self::MISSING,
            border_focused: Self::MISSING,
            text: Self::MISSING,
            text_muted: Self::MISSING,
            text_accent: Self::MISSING,
            icon: Self::MISSING,
            icon_muted: Self::MISSING,
            icon_accent: Self::MISSING,
            title_bar: Self::MISSING,
            status_bar: Self::MISSING,
            panel: Self::MISSING,
            toolbar: Self::MISSING,
            tab_bar: Self::MISSING,
            editor_bg: Self::MISSING,
            editor_fg: Self::MISSING,
            terminal_bg: Self::MISSING,
            vc_added: Self::MISSING,
            vc_modified: Self::MISSING,
            vc_deleted: Self::MISSING,
        }
    }

    #[cfg(test)]
    fn values(&self) -> [u32; 29] {
        [
            self.background,
            self.surface,
            self.elevated_surface,
            self.element_bg,
            self.element_hover,
            self.element_active,
            self.element_selected,
            self.ghost_hover,
            self.ghost_active,
            self.border,
            self.border_variant,
            self.border_focused,
            self.text,
            self.text_muted,
            self.text_accent,
            self.icon,
            self.icon_muted,
            self.icon_accent,
            self.title_bar,
            self.status_bar,
            self.panel,
            self.toolbar,
            self.tab_bar,
            self.editor_bg,
            self.editor_fg,
            self.terminal_bg,
            self.vc_added,
            self.vc_modified,
            self.vc_deleted,
        ]
    }

    fn get_mut(&mut self, field: &str) -> Option<&mut u32> {
        match field {
            "background" => Some(&mut self.background),
            "surface" => Some(&mut self.surface),
            "elevated_surface" => Some(&mut self.elevated_surface),
            "element_bg" => Some(&mut self.element_bg),
            "element_hover" => Some(&mut self.element_hover),
            "element_active" => Some(&mut self.element_active),
            "element_selected" => Some(&mut self.element_selected),
            "ghost_hover" => Some(&mut self.ghost_hover),
            "ghost_active" => Some(&mut self.ghost_active),
            "border" => Some(&mut self.border),
            "border_variant" => Some(&mut self.border_variant),
            "border_focused" => Some(&mut self.border_focused),
            "text" => Some(&mut self.text),
            "text_muted" => Some(&mut self.text_muted),
            "text_accent" => Some(&mut self.text_accent),
            "icon" => Some(&mut self.icon),
            "icon_muted" => Some(&mut self.icon_muted),
            "icon_accent" => Some(&mut self.icon_accent),
            "title_bar" => Some(&mut self.title_bar),
            "status_bar" => Some(&mut self.status_bar),
            "panel" => Some(&mut self.panel),
            "toolbar" => Some(&mut self.toolbar),
            "tab_bar" => Some(&mut self.tab_bar),
            "editor_bg" => Some(&mut self.editor_bg),
            "editor_fg" => Some(&mut self.editor_fg),
            "terminal_bg" => Some(&mut self.terminal_bg),
            "vc_added" => Some(&mut self.vc_added),
            "vc_modified" => Some(&mut self.vc_modified),
            "vc_deleted" => Some(&mut self.vc_deleted),
            _ => None,
        }
    }

    #[cfg(test)]
    fn is_complete(&self) -> bool {
        !self.values().contains(&Self::MISSING)
    }
}

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

/// Field name → Zed theme key.
const KEY_MAP: &[(&str, &str)] = &[
    ("background", "background"),
    ("surface", "surface.background"),
    ("elevated_surface", "elevated_surface.background"),
    ("element_bg", "element.background"),
    ("element_hover", "element.hover"),
    ("element_active", "element.active"),
    ("element_selected", "element.selected"),
    ("ghost_hover", "ghost_element.hover"),
    ("ghost_active", "ghost_element.active"),
    ("border", "border"),
    ("border_variant", "border.variant"),
    ("border_focused", "border.focused"),
    ("text", "text"),
    ("text_muted", "text.muted"),
    ("text_accent", "text.accent"),
    ("icon", "icon"),
    ("icon_muted", "icon.muted"),
    ("icon_accent", "icon.accent"),
    ("title_bar", "title_bar.background"),
    ("status_bar", "status_bar.background"),
    ("panel", "panel.background"),
    ("toolbar", "toolbar.background"),
    ("tab_bar", "tab_bar.background"),
    ("editor_bg", "editor.background"),
    ("editor_fg", "editor.foreground"),
    ("terminal_bg", "terminal.background"),
    ("vc_added", "version_control.added"),
    ("vc_modified", "version_control.modified"),
    ("vc_deleted", "version_control.deleted"),
];

/// Embedded theme families to load, in menu order. GitHub is the default.
const THEME_FILES: &[&str] = &["themes/github.json", "themes/ayu.json", "themes/gruvbox.json"];

/// Defaults for keys some upstream files omit (e.g. Ayu ships without
/// version_control.*). Zed does the same via its fallback themes.
const FALLBACKS: &[(&str, u32)] = &[
    ("vc_added", 0x27a657ff),
    ("vc_modified", 0xd3b020ff),
    ("vc_deleted", 0xe06c76ff),
];

fn parse_hex(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    // Accept #RRGGBB and #RRGGBBAA (Zed files use the latter).
    if hex.len() == 6 {
        u32::from_str_radix(&format!("{hex}ff"), 16).ok()
    } else {
        u32::from_str_radix(hex, 16).ok()
    }
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
    use serde_json::json;
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
            "syntax": style.get("syntax"),
        },
    });
    hl.to_string()
}

impl Theme {
    /// Deserialize into the widget library's highlighter theme.
    pub fn highlight_theme(
        &self,
    ) -> gpui_component::highlighter::HighlightTheme {
        serde_json::from_str(&self.hl_json)
            .unwrap_or_else(|_| {
                // Fall back to the library default if upstream JSON shifts.
                (*gpui_component::highlighter::HighlightTheme::default_dark()).clone()
            })
    }
}

pub fn all() -> &'static [Theme] {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| {
        let mut out = Vec::new();
        for file in THEME_FILES {
            if let Some(data) = crate::AppAssets::get(file) {
                parse_family(std::str::from_utf8(&data.data).unwrap_or(""), &mut out);
            }
        }
        out
    })
}

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
            assert!(t.colors.is_complete(), "theme '{}' is missing tokens", t.name);
        }
    }

    #[test]
    fn parses_hex_with_and_without_alpha() {
        assert_eq!(parse_hex("#282c34"), Some(0x282c34ff));
        assert_eq!(parse_hex("#83899480"), Some(0x83899480));
        assert_eq!(parse_hex("nope"), None);
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
