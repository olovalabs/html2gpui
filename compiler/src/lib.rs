pub mod ast;
pub mod codegen;
pub mod css;
pub mod eval;
pub mod html;
pub mod loader;
pub mod parser;
pub mod script;
pub mod template;
pub mod tokenizer;
pub mod types;
pub mod utils;

pub use ast::{Env, Expr, Flow, FuncDef, IrScript, Stmt, Value};
pub use codegen::{compile_dir, ser_child, ser_elem};
pub use css::{box_expand, color_expr, decl_methods, len_expr, num, parse_css, parse_decls, parse_px_str, tag_defaults};
pub use eval::{binop, call, display, eval, exec, kind, truthy};
pub use html::{collect_children, compile_component_ir, collect_script_text, collect_style_text, gen_element, rewrite_component_tags, strip_uses};
pub use loader::{collect_files_recursive, compile_sources, compile_tree};
pub use parser::Parser;
pub use script::{env_init, env_merge, eval_expr_str, invoke, is_truthy_expr, parse_script};
pub use template::split_template;
pub use tokenizer::{tokenize, Tok};
pub use types::{Child, Compiled, Ctx, IrChild, IrDoc, IrElem, Result, TplPart};
pub use utils::{escape, pascal_case, snake_case, trim_braces};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_template_single_and_double_braces() {
        let parts = split_template("{count}").unwrap();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            TplPart::Var(v) => assert_eq!(v, "count"),
            _ => panic!("expected Var"),
        }

        let parts2 = split_template("{{count}}").unwrap();
        assert_eq!(parts2.len(), 1);
        match &parts2[0] {
            TplPart::Var(v) => assert_eq!(v, "count"),
            _ => panic!("expected Var"),
        }

        let parts3 = split_template("Hello {name}, you have {{count}} items").unwrap();
        assert_eq!(parts3.len(), 5);
    }

    #[test]
    fn test_multiple_functions_in_script() {
        let src = r#"
            let live_stars = 42;
            let dev_name = "Nazmul";
            let revenue = "$89,000";

            function addStar() {
                live_stars++;
            }
            function switchUser() {
                if (dev_name == "Nazmul") {
                    dev_name = "Sarah";
                } else {
                    dev_name = "Nazmul";
                }
            }
            function boostRevenue() {
                if (revenue == "$89,000") {
                    revenue = "$125,000";
                } else {
                    revenue = "$89,000";
                }
            }
        "#;
        let script = parse_script(src).unwrap();
        let mut env = env_init(&script).unwrap();

        assert_eq!(env.get("live_stars"), Some(&Value::Num(42.0)));
        assert_eq!(env.get("dev_name"), Some(&Value::Str("Nazmul".into())));
        assert_eq!(env.get("revenue"), Some(&Value::Str("$89,000".into())));

        invoke(&mut env, &script, "addStar").unwrap();
        assert_eq!(env.get("live_stars"), Some(&Value::Num(43.0)));

        invoke(&mut env, &script, "switchUser").unwrap();
        assert_eq!(env.get("dev_name"), Some(&Value::Str("Sarah".into())));

        invoke(&mut env, &script, "boostRevenue").unwrap();
        assert_eq!(env.get("revenue"), Some(&Value::Str("$125,000".into())));

        invoke(&mut env, &script, "boostRevenue").unwrap();
        assert_eq!(env.get("revenue"), Some(&Value::Str("$89,000".into())));
    }

    #[test]
    fn test_bare_function_invocation_and_mutation() {
        let script = parse_script("let count = 0; function increment() { count++; }").unwrap();
        let mut env = env_init(&script).unwrap();
        assert_eq!(env.get("count"), Some(&Value::Num(0.0)));

        invoke(&mut env, &script, "increment").unwrap();
        assert_eq!(env.get("count"), Some(&Value::Num(1.0)));

        invoke(&mut env, &script, "increment()").unwrap();
        assert_eq!(env.get("count"), Some(&Value::Num(2.0)));

        invoke(&mut env, &script, "count++").unwrap();
        assert_eq!(env.get("count"), Some(&Value::Num(3.0)));
    }
}
