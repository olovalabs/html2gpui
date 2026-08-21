# app

A native desktop GUI app built with [gpui](https://github.com/zed-industries/zed) (Zed's UI framework). The UI is authored as plain HTML files in `root/*.html` and compiled to gpui by the custom `html2gpui` compiler crate.

## Requirements

- Rust (rustup) — `rustc`/`cargo` must be at `%USERPROFILE%\.cargo\bin`
- Windows

## Commands

All commands go through `run.cmd`:

| Command | What it does |
|---|---|
| `run.cmd dev` | Compile `root/*.html` and launch the GUI with **HMR hot-reload** (debug build). Edit any HTML file, save, and the window updates in real time. No restart or Rust recompile needed. |
| `run.cmd build` | Optimized release build → `target\release\app.exe` |
| `run.cmd preview` | Launch the release exe (must run `run.cmd build` first) |
| `cargo check -p app` | Type-check the workspace |
| `cargo test -p html2gpui` | Run compiler tests |

### Bash (Git Bash) equivalents

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # once per shell

./run.cmd dev                          # or:
cargo run -p app                       # dev with HMR
cargo build --release -p app           # release build
./target/release/app.exe               # launch release
```

## Project layout

```
app/        GUI app (gpui) — debug mode includes the HMR watcher
compiler/   html2gpui — compiles root/*.html into gpui code
root/       Your UI source: app.html is the entry component
run.cmd     dev / build / preview wrapper
```

## Writing UI

Each `.html` file in `root/` becomes one component (filename → component name). `app.html` is the entry point.

```html
@use Info from "./info.html"

<html>
    <h1>Hello world</h1>
    <Info />
</html>

<style>
h1 {
    color: yellow;
    background-color: green;
}
</style>
```

- `@use Name from "./file.html"` imports another component
- `<Name />` places it in the layout
- Style with `<style>` blocks (tag selectors like `h1 {}` and class selectors like `.card {}`) or inline `style="..."` attributes

## HMR (hot-reload)

`run.cmd dev` runs the app in debug mode, which:

1. Watches all `root/*.html` files (~3 checks/sec)
2. On save, recompiles them in-memory and redraws the window instantly
3. If your HTML has an error (bad `@use`, syntax error, etc.), the window shows a red "Compile error" screen with the message — fix the file and it recovers automatically

Only the HTML is hot-reloaded. If you edit Rust code (`app/src`, `compiler/src`), stop the app (`Ctrl+C`) and run `run.cmd dev` again.

## Release vs debug

- **Debug (`run.cmd dev`)**: HMR watcher active, slower rendering
- **Release (`run.cmd build`)**: HTML is compiled into the binary at build time, no watcher
