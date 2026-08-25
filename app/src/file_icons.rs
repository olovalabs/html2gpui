//! File icons resolved from the file extension / name, using Zed's default
//! icon theme ("Zed (Default)").
//!
//! Ported from zed-industries/zed:
//! - `crates/theme/src/icon_theme.rs`  (association tables)
//! - `crates/file_icons/src/file_icons.rs` (lookup algorithm)
//!
//! The SVG assets live in `app/assets/file_icons/` and are embedded via
//! rust-embed; paths here are asset-source paths like `file_icons/rust.svg`.
//!
//! Icons are multicolor, so render them with `gpui::img()` — gpui's `svg()`
//! element only paints a single tint color.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Exact file names → icon type.
const FILE_STEMS: &[(&str, &str)] = &[
    ("Containerfile", "docker"),
    ("Dockerfile", "docker"),
    ("Podfile", "ruby"),
    ("Procfile", "heroku"),
];

/// Name/extension suffixes → icon type. Matched against progressively
/// dot-stripped suffixes of the file name (see [`type_for_suffix`]).
#[rustfmt::skip]
const FILE_SUFFIXES: &[(&str, &str)] = &[
    ("astro", "astro"),
    ("aac", "audio"), ("flac", "audio"), ("m4a", "audio"), ("mka", "audio"), ("mp3", "audio"), ("ogg", "audio"), ("opus", "audio"), ("wav", "audio"), ("wma", "audio"), ("wv", "audio"),
    ("bak", "backup"),
    ("bal", "ballerina"),
    ("bicep", "bicep"),
    ("lockb", "bun"),
    ("c", "c"), ("h", "c"),
    ("cairo", "cairo"),
    ("handlebars", "code"), ("metadata", "code"), ("rkt", "code"), ("scm", "code"),
    ("coffee", "coffeescript"),
    ("c++", "cpp"), ("h++", "cpp"), ("cc", "cpp"), ("cpp", "cpp"), ("cppm", "cpp"), ("cxx", "cpp"), ("hh", "cpp"), ("hpp", "cpp"), ("hxx", "cpp"), ("inl", "cpp"), ("ixx", "cpp"),
    ("cr", "crystal"), ("ecr", "crystal"),
    ("cs", "csharp"),
    ("csproj", "csproj"),
    ("css", "css"), ("pcss", "css"), ("postcss", "css"),
    ("cue", "cue"),
    ("dart", "dart"),
    ("diff", "diff"),
    ("docker-compose.yml", "docker"), ("docker-compose.yaml", "docker"), ("compose.yml", "docker"), ("compose.yaml", "docker"),
    ("doc", "document"), ("docx", "document"), ("mdx", "document"), ("odp", "document"), ("ods", "document"), ("odt", "document"), ("pdf", "document"), ("ppt", "document"), ("pptx", "document"), ("rtf", "document"), ("txt", "document"), ("xls", "document"), ("xlsx", "document"),
    ("editorconfig", "editorconfig"),
    ("eex", "elixir"), ("ex", "elixir"), ("exs", "elixir"), ("heex", "elixir"), ("leex", "elixir"), ("neex", "elixir"),
    ("elm", "elm"),
    ("Emakefile", "erlang"), ("app.src", "erlang"), ("erl", "erlang"), ("escript", "erlang"), ("hrl", "erlang"), ("rebar.config", "erlang"), ("xrl", "erlang"), ("yrl", "erlang"),
    ("eslint.config.cjs", "eslint"), ("eslint.config.cts", "eslint"), ("eslint.config.js", "eslint"), ("eslint.config.mjs", "eslint"), ("eslint.config.mts", "eslint"), ("eslint.config.ts", "eslint"), ("eslintrc", "eslint"), ("eslintrc.js", "eslint"), ("eslintrc.json", "eslint"),
    ("otf", "font"), ("ttf", "font"), ("woff", "font"), ("woff2", "font"),
    ("fs", "fsharp"),
    ("fsproj", "fsproj"),
    ("gitlab-ci.yml", "gitlab"), ("gitlab-ci.yaml", "gitlab"),
    ("gleam", "gleam"),
    ("go", "go"), ("mod", "go"), ("work", "go"),
    ("gql", "graphql"), ("graphql", "graphql"), ("graphqls", "graphql"),
    ("hs", "haskell"),
    ("hcl", "hcl"),
    ("helmfile.yaml", "helm"), ("helmfile.yml", "helm"), ("Chart.yaml", "helm"), ("Chart.yml", "helm"), ("Chart.lock", "helm"), ("values.yaml", "helm"), ("values.yml", "helm"), ("requirements.yaml", "helm"), ("requirements.yml", "helm"), ("tpl", "helm"),
    ("htm", "html"), ("html", "html"),
    ("avif", "image"), ("bmp", "image"), ("gif", "image"), ("heic", "image"), ("heif", "image"), ("ico", "image"), ("j2k", "image"), ("jfif", "image"), ("jp2", "image"), ("jpeg", "image"), ("jpg", "image"), ("jxl", "image"), ("png", "image"), ("psd", "image"), ("qoi", "image"), ("svg", "image"), ("tiff", "image"), ("webp", "image"),
    ("ipynb", "ipynb"),
    ("java", "java"),
    ("cjs", "javascript"), ("js", "javascript"), ("mjs", "javascript"),
    ("json", "json"), ("jsonc", "json"),
    ("jl", "julia"),
    ("kdl", "kdl"),
    ("kt", "kotlin"),
    ("lock", "lock"),
    ("log", "log"),
    ("lua", "lua"),
    ("luau", "luau"),
    ("markdown", "markdown"), ("md", "markdown"),
    ("metal", "metal"),
    ("nim", "nim"), ("nims", "nim"), ("nimble", "nim"),
    ("nix", "nix"),
    ("ml", "ocaml"), ("mli", "ocaml"), ("mlx", "ocaml"),
    ("odin", "odin"),
    ("php", "php"),
    ("prettier.config.cjs", "prettier"), ("prettier.config.js", "prettier"), ("prettier.config.mjs", "prettier"), ("prettierignore", "prettier"), ("prettierrc", "prettier"), ("prettierrc.cjs", "prettier"), ("prettierrc.js", "prettier"), ("prettierrc.json", "prettier"), ("prettierrc.json5", "prettier"), ("prettierrc.mjs", "prettier"), ("prettierrc.toml", "prettier"), ("prettierrc.yaml", "prettier"), ("prettierrc.yml", "prettier"),
    ("prisma", "prisma"),
    ("pp", "puppet"),
    ("py", "python"),
    ("r", "r"), ("R", "r"),
    ("cjsx", "react"), ("ctsx", "react"), ("jsx", "react"), ("mjsx", "react"), ("mtsx", "react"), ("tsx", "react"),
    ("roc", "roc"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("sass", "sass"), ("scss", "sass"),
    ("scala", "scala"), ("sc", "scala"),
    ("conf", "settings"), ("ini", "settings"),
    ("sol", "solidity"),
    ("accdb", "storage"), ("csv", "storage"), ("dat", "storage"), ("db", "storage"), ("dbf", "storage"), ("dll", "storage"), ("fmp", "storage"), ("fp7", "storage"), ("frm", "storage"), ("gdb", "storage"), ("ib", "storage"), ("ldf", "storage"), ("mdb", "storage"), ("mdf", "storage"), ("myd", "storage"), ("myi", "storage"), ("pdb", "storage"), ("psv", "storage"), ("RData", "storage"), ("rdata", "storage"), ("sav", "storage"), ("sdf", "storage"), ("sql", "storage"), ("sqlite", "storage"), ("ssv", "storage"), ("tsv", "storage"),
    ("stylelint.config.cjs", "stylelint"), ("stylelint.config.js", "stylelint"), ("stylelint.config.mjs", "stylelint"), ("stylelintignore", "stylelint"), ("stylelintrc", "stylelint"), ("stylelintrc.cjs", "stylelint"), ("stylelintrc.js", "stylelint"), ("stylelintrc.json", "stylelint"), ("stylelintrc.mjs", "stylelint"), ("stylelintrc.yaml", "stylelint"), ("stylelintrc.yml", "stylelint"),
    ("surql", "surrealql"),
    ("svelte", "svelte"),
    ("swift", "swift"),
    ("tcl", "tcl"),
    ("hbs", "template"), ("plist", "template"), ("xml", "template"),
    ("bash", "terminal"), ("bash_aliases", "terminal"), ("bash_login", "terminal"), ("bash_logout", "terminal"), ("bash_profile", "terminal"), ("bashrc", "terminal"), ("brushrc", "terminal"), ("fish", "terminal"), ("nu", "terminal"), ("profile", "terminal"), ("ps1", "terminal"), ("sh", "terminal"), ("zlogin", "terminal"), ("zlogout", "terminal"), ("zprofile", "terminal"), ("zsh", "terminal"), ("zsh_aliases", "terminal"), ("zsh_histfile", "terminal"), ("zsh_history", "terminal"), ("zshenv", "terminal"), ("zshrc", "terminal"),
    ("tf", "terraform"), ("tfvars", "terraform"),
    ("toml", "toml"),
    ("cts", "typescript"), ("mts", "typescript"), ("ts", "typescript"),
    ("v", "v"), ("vsh", "v"), ("vv", "v"),
    ("COMMIT_EDITMSG", "vcs"), ("EDIT_DESCRIPTION", "vcs"), ("MERGE_MSG", "vcs"), ("NOTES_EDITMSG", "vcs"), ("TAG_EDITMSG", "vcs"), ("gitattributes", "vcs"), ("gitignore", "vcs"), ("gitkeep", "vcs"), ("gitmodules", "vcs"),
    ("vbproj", "vbproj"),
    ("avi", "video"), ("m4v", "video"), ("mkv", "video"), ("mov", "video"), ("mp4", "video"), ("webm", "video"), ("wmv", "video"),
    ("sln", "vs_sln"),
    ("suo", "vs_suo"),
    ("vue", "vue"),
    ("vy", "vyper"), ("vyi", "vyper"),
    ("wgsl", "wgsl"),
    ("yaml", "yaml"), ("yml", "yaml"),
    ("zig", "zig"),
];

/// Icon type → SVG asset path.
const TYPE_ICONS: &[(&str, &str)] = &[
    ("astro", "file_icons/astro.svg"),
    ("audio", "file_icons/audio.svg"),
    ("ballerina", "file_icons/ballerina.svg"),
    ("bicep", "file_icons/file.svg"),
    ("bun", "file_icons/bun.svg"),
    ("c", "file_icons/c.svg"),
    ("cairo", "file_icons/cairo.svg"),
    ("code", "file_icons/code.svg"),
    ("coffeescript", "file_icons/coffeescript.svg"),
    ("cpp", "file_icons/cpp.svg"),
    ("crystal", "file_icons/file.svg"),
    ("csharp", "file_icons/file.svg"),
    ("csproj", "file_icons/file.svg"),
    ("css", "file_icons/css.svg"),
    ("cue", "file_icons/file.svg"),
    ("dart", "file_icons/dart.svg"),
    ("default", "file_icons/file.svg"),
    ("diff", "file_icons/diff.svg"),
    ("docker", "file_icons/docker.svg"),
    ("document", "file_icons/book.svg"),
    ("editorconfig", "file_icons/editorconfig.svg"),
    ("elixir", "file_icons/elixir.svg"),
    ("elm", "file_icons/elm.svg"),
    ("erlang", "file_icons/erlang.svg"),
    ("eslint", "file_icons/eslint.svg"),
    ("font", "file_icons/font.svg"),
    ("fsharp", "file_icons/fsharp.svg"),
    ("fsproj", "file_icons/file.svg"),
    ("gitlab", "file_icons/gitlab.svg"),
    ("gleam", "file_icons/gleam.svg"),
    ("go", "file_icons/go.svg"),
    ("graphql", "file_icons/graphql.svg"),
    ("haskell", "file_icons/haskell.svg"),
    ("hcl", "file_icons/hcl.svg"),
    ("helm", "file_icons/helm.svg"),
    ("heroku", "file_icons/heroku.svg"),
    ("html", "file_icons/html.svg"),
    ("image", "file_icons/image.svg"),
    ("ipynb", "file_icons/jupyter.svg"),
    ("java", "file_icons/java.svg"),
    ("javascript", "file_icons/javascript.svg"),
    ("json", "file_icons/code.svg"),
    ("julia", "file_icons/julia.svg"),
    ("kdl", "file_icons/kdl.svg"),
    ("kotlin", "file_icons/kotlin.svg"),
    ("lock", "file_icons/lock.svg"),
    ("log", "file_icons/info.svg"),
    ("lua", "file_icons/lua.svg"),
    ("luau", "file_icons/luau.svg"),
    ("markdown", "file_icons/book.svg"),
    ("metal", "file_icons/metal.svg"),
    ("nim", "file_icons/nim.svg"),
    ("nix", "file_icons/nix.svg"),
    ("ocaml", "file_icons/ocaml.svg"),
    ("odin", "file_icons/odin.svg"),
    ("phoenix", "file_icons/phoenix.svg"),
    ("php", "file_icons/php.svg"),
    ("prettier", "file_icons/prettier.svg"),
    ("prisma", "file_icons/prisma.svg"),
    ("puppet", "file_icons/puppet.svg"),
    ("python", "file_icons/python.svg"),
    ("r", "file_icons/r.svg"),
    ("react", "file_icons/react.svg"),
    ("roc", "file_icons/roc.svg"),
    ("ruby", "file_icons/ruby.svg"),
    ("rust", "file_icons/rust.svg"),
    ("sass", "file_icons/sass.svg"),
    ("scala", "file_icons/scala.svg"),
    ("settings", "file_icons/settings.svg"),
    ("solidity", "file_icons/file.svg"),
    ("storage", "file_icons/database.svg"),
    ("stylelint", "file_icons/javascript.svg"),
    ("surrealql", "file_icons/surrealql.svg"),
    ("svelte", "file_icons/html.svg"),
    ("swift", "file_icons/swift.svg"),
    ("tcl", "file_icons/tcl.svg"),
    ("template", "file_icons/html.svg"),
    ("terminal", "file_icons/terminal.svg"),
    ("terraform", "file_icons/terraform.svg"),
    ("toml", "file_icons/toml.svg"),
    ("typescript", "file_icons/typescript.svg"),
    ("v", "file_icons/v.svg"),
    ("vbproj", "file_icons/file.svg"),
    ("vcs", "file_icons/git.svg"),
    ("video", "file_icons/video.svg"),
    ("vs_sln", "file_icons/file.svg"),
    ("vs_suo", "file_icons/file.svg"),
    ("vue", "file_icons/vue.svg"),
    ("vyper", "file_icons/vyper.svg"),
    ("wgsl", "file_icons/wgsl.svg"),
    ("yaml", "file_icons/yaml.svg"),
    ("zig", "file_icons/zig.svg"),
];

fn stems() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| FILE_STEMS.iter().copied().collect())
}

fn suffixes() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| FILE_SUFFIXES.iter().copied().collect())
}

fn type_icons() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| TYPE_ICONS.iter().copied().collect())
}

fn icon_for_type(ty: &str) -> Option<&'static str> {
    type_icons().get(ty).copied()
}

fn type_for_suffix(suffix: &str) -> Option<&'static str> {
    stems()
        .get(suffix)
        .or_else(|| suffixes().get(suffix))
        .copied()
}

/// Returns the asset path of the icon for a file path, falling back to
/// Zed's default file icon (`file.svg`). Mirrors Zed's lookup order:
///
/// 1. exact file name (`Dockerfile`, `.gitignore` handled below)
/// 2. progressively dot-stripped name suffixes, so `auth.module.js` tries
///    `module.js`, then `js`; hidden files try their bare name
///    (`.gitignore` → `gitignore`), multi-part extensions work too
///    (`Component.stories.tsx` → `stories.tsx`)
/// 3. plain extension
/// 4. `"default"` → `file_icons/file.svg`
pub fn icon_for(path: &Path) -> &'static str {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return "file_icons/file.svg";
    };

    let mut candidate = name;
    loop {
        if let Some(ty) = type_for_suffix(candidate) {
            if let Some(icon) = icon_for_type(ty) {
                return icon;
            }
        }
        match candidate.split_once('.') {
            Some((_, rest)) if !rest.is_empty() => candidate = rest,
            _ => break,
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if let Some(icon) = type_for_suffix(ext).and_then(icon_for_type) {
            return icon;
        }
    }

    icon_for_type("default").unwrap_or("file_icons/file.svg")
}

pub const FOLDER_COLLAPSED: &str = "file_icons/folder.svg";
pub const FOLDER_EXPANDED: &str = "file_icons/folder_open.svg";
pub const CHEVRON_COLLAPSED: &str = "file_icons/chevron_right.svg";
pub const CHEVRON_EXPANDED: &str = "file_icons/chevron_down.svg";

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(name: &str) -> &'static str {
        icon_for(Path::new(name))
    }

    #[test]
    fn by_extension() {
        assert_eq!(icon("main.rs"), "file_icons/rust.svg");
        assert_eq!(icon("index.js"), "file_icons/javascript.svg");
        assert_eq!(icon("App.tsx"), "file_icons/react.svg");
        assert_eq!(icon("style.scss"), "file_icons/sass.svg");
        assert_eq!(icon("data.csv"), "file_icons/database.svg");
    }

    #[test]
    fn by_file_name() {
        assert_eq!(icon("Dockerfile"), "file_icons/docker.svg");
        assert_eq!(icon("Cargo.toml"), "file_icons/toml.svg");
    }

    #[test]
    fn hidden_files() {
        assert_eq!(icon(".gitignore"), "file_icons/git.svg");
        assert_eq!(icon(".zshrc"), "file_icons/terminal.svg");
    }

    #[test]
    fn multipart_names() {
        assert_eq!(icon("auth.module.js"), "file_icons/javascript.svg");
        assert_eq!(icon("Button.stories.tsx"), "file_icons/react.svg");
        assert_eq!(
            icon("eslint.config.mjs"),
            "file_icons/eslint.svg",
            "full-name suffix beats plain extension"
        );
    }

    #[test]
    fn fallback_default() {
        assert_eq!(icon("unknown.xyz"), "file_icons/file.svg");
    }
}
