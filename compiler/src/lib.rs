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
pub use html::{collect_children, compile_component_ir, collect_script_text, collect_style_text, gen_element, rewrite_component_tags, strip_script_imports};
pub use loader::{collect_files_recursive, compile_sources, compile_tree};
pub use parser::Parser;
pub use script::{env_init, env_merge, eval_expr, eval_expr_str, invoke, is_truthy_expr, parse_script};
pub use template::split_template;
pub use tokenizer::{tokenize, Tok};
pub use types::{Child, Compiled, Ctx, IrChild, IrDoc, IrElem, Result, TplPart};
pub use utils::{escape, pascal_case, snake_case, trim_braces};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Child;

    #[test]
    fn test_jsx_expressions_in_templates() {
        // {1 + 1}
        let parts = split_template("{1 + 1}").unwrap();
        assert_eq!(parts.len(), 1);
        assert!(matches!(parts[0], TplPart::Expr(Expr::Binary(_, _, _))));

        // ternary
        let parts = split_template("{count > 10 ? \"high\" : \"low\"}").unwrap();
        assert!(parts.iter().any(
            |p| matches!(p, TplPart::Expr(_))
        ));

        // function call
        let parts = split_template("{console.log()}").unwrap();
        assert!(matches!(parts[0], TplPart::Expr(Expr::Call(_, _))));

        // mixed literal + expression
        let parts = split_template("Total: {price * 2} USD").unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], TplPart::Lit("Total: ".into()));
        assert!(matches!(parts[1], TplPart::Expr(_)));
        assert_eq!(parts[2], TplPart::Lit(" USD".into()));

        // bare var still Var
        let parts = split_template("{count}").unwrap();
        assert_eq!(parts[0], TplPart::Var("count".into()));

        // double braces still work
        assert!(split_template("{{count}}").is_some());

        // invalid expression stays plain text (no var parts -> None)
        let parts = split_template("{1 + }");
        assert!(
            parts.is_none()
                || parts
                    .unwrap()
                    .iter()
                    .all(|p| matches!(p, TplPart::Lit(_)))
        );

        // evaluating an expr against env works end to end
        let script = parse_script("let price = 5;").unwrap();
        let env = env_init(&script).unwrap();
        if let TplPart::Expr(e) = &split_template("{price * 2 + 1}").unwrap()[0] {
            assert_eq!(eval_expr(e, &env).unwrap(), Value::Num(11.0));
        } else {
            panic!("expected Expr part");
        }
    }

    #[test]
    fn test_jsx_expr_compile_pipeline() {
        use crate::loader::compile_sources;
        let files = vec![
            ("app.html".to_string(), r#"<script>
  import Demo from "./demo.html";
</script>
<div><Demo /></div>"#.to_string()),
            ("demo.html".to_string(), r#"<script>
  import "./demo.css";
  let count = 4;
</script>
<div class="box">
  <p>{1 + 1}</p>
  <p>{count > 3 ? "big" : "small"}</p>
</div>"#.to_string()),
            ("demo.css".to_string(), ".box { padding: 8px; }".to_string()),
        ];
        let (doc, warnings) = compile_sources(&files).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");

        let Child::Div(root) = &doc.comps["demo"].children[0] else {
            panic!("expected div");
        };
        // <p>{1 + 1}</p> and <p>{count > 3 ? ... : ...}</p>
        let Child::Div(p1) = &root.children[0] else {
            panic!("expected p div child");
        };
        let Child::Tpl(t1) = &p1.children[0] else {
            panic!("expected tpl inside first p");
        };
        assert!(matches!(t1[0], TplPart::Expr(_)));
        let Child::Div(p2) = &root.children[1] else {
            panic!("expected second p div child");
        };
        let Child::Tpl(t2) = &p2.children[0] else {
            panic!("expected tpl inside second p");
        };
        assert!(matches!(t2[0], TplPart::Expr(_)));

        // and evaluates correctly through the runtime helper
        let env = html2gpui_env(&doc);
        let v = match &t2[0] {
            TplPart::Expr(e) => eval_expr(e, &env).unwrap(),
            other => panic!("expected expr, got {other:?}"),
        };
        assert_eq!(v, Value::Str("big".into()));
    }

    fn html2gpui_env(doc: &IrDoc) -> Env {
        env_init(&doc.script).unwrap()
    }
}