use std::collections::BTreeMap;
use crate::ast::{Env, Expr, Flow, IrScript, Value};
use crate::eval::{eval, exec, truthy};
use crate::parser::Parser;
use crate::tokenizer::tokenize;
use crate::types::Result;

pub fn parse_script(src: &str) -> Result<IrScript> {
    let toks = tokenize(src)?;
    Parser { toks, pos: 0 }.script()
}

pub fn env_init(script: &IrScript) -> Result<Env> {
    let mut env: Env = BTreeMap::new();
    let funcs = &script.funcs;
    for (name, e) in &script.vars {
        let mut empty_locals = BTreeMap::new();
        let Flow::Value(v) = eval(e, &mut empty_locals, &mut env, funcs).map_err(|err| format!("{name}: {err}"))?
        else {
            return Err(format!("{name}: unexpected return"));
        };
        env.insert(name.clone(), v);
    }
    Ok(env)
}

pub fn env_merge(old: &mut Env, script: &IrScript) -> Result<()> {
    let mut fresh: Env = BTreeMap::new();
    let funcs = &script.funcs;
    for (name, e) in &script.vars {
        let v = match old.get(name) {
            Some(existing) => existing.clone(),
            None => {
                let mut empty_locals = BTreeMap::new();
                let Flow::Value(v) =
                    eval(e, &mut empty_locals, old, funcs).map_err(|err| format!("{name}: {err}"))?
                else {
                    return Err(format!("{name}: unexpected return"));
                };
                v
            }
        };
        fresh.insert(name.clone(), v);
    }
    *old = fresh;
    Ok(())
}

pub fn invoke(env: &mut Env, script: &IrScript, src: &str) -> Result<Value> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let mut result = Value::Null;
    loop {
        p.eat_newlines();
        if p.peek().is_none() {
            break;
        }
        let stmt = p.expr_full()?;
        p.stmt_end()?;
        let mut empty_locals = BTreeMap::new();
        match exec(std::slice::from_ref(&stmt), &mut empty_locals, env, &script.funcs)? {
            Flow::Return(v) | Flow::Value(v) => result = v,
            Flow::Next => {}
        }
    }
    Ok(result)
}

pub fn eval_expr(e: &Expr, env: &Env) -> Result<Value> {
    let mut env_clone = env.clone();
    let mut empty_locals = BTreeMap::new();
    let funcs = BTreeMap::new();
    let Flow::Value(v) = eval(e, &mut empty_locals, &mut env_clone, &funcs)? else {
        return Err("unexpected return".into());
    };
    Ok(v)
}

pub fn eval_expr_str(env: &Env, script: &IrScript, src: &str) -> Result<Value> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.expr()?;
    let mut empty_locals = BTreeMap::new();
    let mut env_clone = env.clone();
    let Flow::Value(v) = eval(&e, &mut empty_locals, &mut env_clone, &script.funcs)? else {
        return Err("unexpected return".into());
    };
    Ok(v)
}

pub fn is_truthy_expr(env: &Env, script: &IrScript, src: &str) -> bool {
    eval_expr_str(env, script, src).map(|v| truthy(&v)).unwrap_or(false)
}
