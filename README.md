<table>
  <tr>
    <img width="500" height="150" alt="html2gpui-logo" src="https://github.com/user-attachments/assets/a44c855c-e9ad-40cb-b481-f87731a0cd3e" />
  </tr>
</table>


# html2gpui ⚡

> Compile declarative HTML, CSS, and reactive JS components directly into high-performance, GPU-accelerated native desktop applications powered by [Zed's GPUI](https://github.com/zed-industries/zed).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/Engine-Zed%20GPUI%200.2-purple.svg)](https://github.com/zed-industries/zed)
[![Cross-Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-green.svg)](https://github.com/olovalabs/html2gpui)

---

## ✨ Features

- ⚡ **Sub-15ms In-Memory HMR**: Edit any `.html` or `.css` file and watch the desktop GUI update instantly in real time without restarting the Rust app.
- 🌐 **100% Cross-Platform**: Runs natively on **macOS**, **Linux**, and **Windows** with hardware-accelerated GPU rendering.
- 🧩 **Component Architecture**: Modular component imports with `@use Component from "./path"`, custom alias imports, and props passing (`<Card title="Hello" count={count} />`).
- 💡 **Reactive State Engine**: Write JS-like scripts with `let`, `function`, `onclick={fn}`, and reactive text interpolation (`{var}` and `{{var}}`).
- 🎨 **Shadcn UI & Global CSS**: Centralized design system in `root/global.css` automatically inherited by all components with local `<style>` overrides.
- 🖼️ **Hardware-Accelerated SVGs**: Drop raw `<svg src="icons/name.svg" />` files directly into templates with custom dimensions and color tinting.
- 📜 **Bounded Viewport Scrolling**: Smart browser-grade scroll engine with dynamic viewport measurement and proportional scrollbar thumb indicators.
- 📁 **Universal Folder Structure**: Create components and stylesheets at any directory depth inside `root/`.

---

## 🚀 Quick Start

### Prerequisites
- [Rust](https://rustup.rs/) (rustup toolchain `1.80+`)

### Running Development Server (with Live HMR)

#### 🍏 macOS & 🐧 Linux
```bash
chmod +x run.sh
./run.sh dev
```

#### 🪟 Windows (Git Bash or CMD)
```bash
./run.cmd dev
```

#### 💻 Universal (Any OS via Cargo)
```bash
cargo run -p app
```

---

### Building for Production (Optimized Release)

#### 🍏 macOS & 🐧 Linux
```bash
./run.sh build
./target/release/app
```

#### 🪟 Windows
```bash
./run.cmd build
./target/release/app.exe
```

---

## 📂 Project Structure

```
root/                          # UI Source (HTML, CSS, Assets)
├── app.html                   # Root entry point & layout router
├── global.css                 # Global design system & utility classes
├── navbar.html                # Top navigation header component
├── sidebar.html               # Left sidebar navigation component
├── dashboard.html             # Main dashboard analytics view
├── props_test.html            # Interactive props passing test lab
├── about.html                 # State management showcase view
├── info.html                  # Architecture specifications view
├── settings.html              # Configuration & settings view
├── components/                # Reusable UI components
│   ├── footer.html
│   ├── stat_box.html
│   └── user_card.html
└── icons/                     # Vector SVG icons
    ├── dashboard.svg
    ├── layers.svg
    ├── terminal.svg
    └── settings.svg

compiler/                      # html2gpui crate (AST parser & interpreter)
├── src/
│   ├── ast.rs                 # Script language AST nodes
│   ├── codegen.rs             # Static Rust code serializer
│   ├── css.rs                 # CSS parser & GPUI style mapper
│   ├── eval.rs                # Scoped expression evaluator & executor
│   ├── html.rs                # HTML DOM parser & component tag rewriter
│   ├── loader.rs              # Recursive file scanner & module resolver
│   ├── parser.rs              # Recursive descent script parser
│   ├── script.rs              # Public runtime script facade
│   ├── template.rs            # Single & double brace template parser
│   ├── tokenizer.rs           # Lexical tokenizer
│   ├── types.rs               # Core IR types (IrDoc, IrElem, IrChild)
│   ├── utils.rs               # String & casing helpers
│   └── lib.rs                 # Main module wiring & unit tests

app/                           # Native GPUI application shell
└── src/
    └── main.rs                # GPU surface mounting & live HMR watcher
```

---

## 📝 Writing Components

### 1. Component with Reactive State
```html
<script>
  let count = 0;

  function increment() {
    count++;
  }

  function decrement() {
    if (count > 0) {
      count--;
    }
  }
</script>

<div class="counter-card">
  <h3>Count: {count}</h3>
  <div class="row">
    <button class="btn" onclick={decrement}>-</button>
    <button class="btn btn-primary" onclick={increment}>+</button>
  </div>
</div>
```

### 2. Passing Props to Reusable Components
```html
<!-- Parent View -->
@use UserCard from "./components/user_card.html"

<script>
  let stars = 42;
</script>

<div>
  <UserCard name="Nazmul Hossain" role="Lead Architect" stars={stars} />
  <button class="btn" onclick="stars++">+ Star</button>
</div>
```

```html
<!-- root/components/user_card.html -->
<div class="card">
  <h4>{name}</h4>
  <p>{role}</p>
  <span>★ {stars} Stars</span>
</div>
```

### 3. Conditional Rendering (Show / Hide)
```html
<script>
  let is_open = false;
</script>

<button class="btn" onclick="is_open = !is_open">Toggle</button>

<div if={is_open} class="drawer">
  <p>Conditionally mounted when is_open is true!</p>
</div>
```

---

## 🧪 Testing the Compiler

Run compiler unit tests across all platforms:
```bash
cargo test -p html2gpui
```

---

## 📜 License

MIT © [Olova Labs](https://github.com/olovalabs)
