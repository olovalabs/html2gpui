# gpui editor

Native code editor: file tree + [gpui-component](https://github.com/longbridge/gpui-component) `Input` in **code editor** mode (rope buffer, tree-sitter highlighting, search).

```
cargo run -p app
```

Starts with **no project loaded** (VS Code-style welcome screen):

- **Open Folder** — pick a folder to browse in the explorer
- **Open File** (`Ctrl+O`) — open a single file
- **New File** (`Ctrl+N`) — untitled buffer; `Ctrl+S` opens Save As
- **Ctrl+S** save · **Ctrl+F** search (from the editor library)

## Syntax highlighting — Zed's exact palette

Default theme: **GitHub Dark** (from
[PyaeSoneAungRgn/github-zed-theme](https://github.com/PyaeSoneAungRgn/github-zed-theme)),
plus Ayu and Gruvbox families. The JSONs in `app/assets/themes/` are verbatim
Zed theme files consumed two ways:

1. UI chrome colors → `src/theme.rs` tokens
2. the full `syntax` token table (46 entries) + editor colors → forwarded
   untouched into gpui-component's `HighlightTheme`, whose schema is
   explicitly Zed-compatible. Tree-sitter captures (`keyword`, `function`,
   `comment.doc`, …) are painted with exactly Zed's colors; switching themes
   via **View → Theme** re-skins chrome *and* syntax together.

## Language servers

`src/main.rs` maps each language to the LSP binary Zed uses by default
(rust-analyzer, gopls, basedpyright, clangd, typescript-language-server,
zls, …), detects it on PATH and reports readiness in the status bar when you
open a matching file. Wiring an actual server process (JSON-RPC stdio →
diagnostics/completions via the library's `DiagnosticSet` / `Lsp` provider
APIs) is the next milestone.

## Fonts — Zed's shipped set

`app/assets/fonts/` contains the exact TTFs Zed ships: **IBM Plex Sans**
(UI, 4 styles) and **Lilex** (code buffer, 4 styles), with their licenses
(`LICENSE-IBMPlex.txt`, `LICENSE-Lilex-OFL.txt`). At startup every embedded
TTF is registered with GPUI's text system (`load_embedded_fonts`, mirroring
Zed's approach); the widget library is then pointed at them
(`mono_font_family = Lilex` for the editor, `font_family = IBM Plex Sans`
for UI) and re-applied after every theme switch.

## File icons

Explorer icons come from Zed's default icon theme — SVGs in
`app/assets/file_icons/` and the extension→icon tables in `src/file_icons.rs`,
ported from Zed's `crates/theme/src/icon_theme.rs`. Unknown extensions fall
back to Zed's default file icon. Activity-bar glyphs are Zed's own icons
(`app/assets/ui_icons/`).

Note: colorful file/folder icons are rendered with `gpui::img()` because
gpui's `svg()` element paints a single tint color; UI glyphs intentionally use
tinted `svg()` so they follow the theme.
