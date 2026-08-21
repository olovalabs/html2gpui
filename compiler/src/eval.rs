use std::collections::BTreeMap;
use crate::ast::{Env, Expr, Flow, FuncDef, Stmt, Value};
use crate::types::Result;

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Num(n) => *n != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::Null => false,
    }
}

pub fn binop(op: &str, a: &Value, b: &Value) -> Result<Value> {
    use Value::*;
    let out = match (op, a, b) {
        ("+", Str(x), y) => Str(format!("{x}{}", display(y))),
        ("+", x, Str(y)) => Str(format!("{}{y}", display(x))),
        ("+", Num(x), Num(y)) => Num(x + y),
        ("-", Num(x), Num(y)) => Num(x - y),
        ("*", Num(x), Num(y)) => Num(x * y),
        ("/", Num(x), Num(y)) => {
            if *y == 0.0 {
                return Err("division by zero".into());
            }
            Num(x / y)
        }
        ("%", Num(x), Num(y)) => {
            if *y == 0.0 {
                return Err("modulo by zero".into());
            }
            Num(x % y)
        }
        ("==", x, y) => Bool(x == y),
        ("!=", x, y) => Bool(x != y),
        ("<", Num(x), Num(y)) => Bool(x < y),
        (">", Num(x), Num(y)) => Bool(x > y),
        ("<=", Num(x), Num(y)) => Bool(x <= y),
        (">=", Num(x), Num(y)) => Bool(x >= y),
        ("&&", x, y) => Bool(truthy(x) && truthy(y)),
        ("||", x, y) => Bool(truthy(x) || truthy(y)),
        ("<", Str(x), Str(y)) => Bool(x < y),
        (">", Str(x), Str(y)) => Bool(x > y),
        ("<=", Str(x), Str(y)) => Bool(x <= y),
        (">=", Str(x), Str(y)) => Bool(x >= y),
        _ => return Err(format!("bad types for `{op}`: {} {}", kind(a), kind(b))),
    };
    Ok(out)
}

pub fn kind(v: &Value) -> &'static str {
    match v {
        Value::Num(_) => "number",
        Value::Str(_) => "string",
        Value::Bool(_) => "bool",
        Value::Null => "null",
    }
}

pub fn display(v: &Value) -> String {
    match v {
        Value::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Value::Str(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
    }
}

pub fn eval(
    e: &Expr,
    locals: &mut BTreeMap<String, Value>,
    globals: &mut Env,
    funcs: &BTreeMap<String, FuncDef>,
) -> Result<Flow> {
    let v = match e {
        Expr::Num(n) => Value::Num(*n),
        Expr::Str(s) => Value::Str(s.clone()),
        Expr::Bool(b) => Value::Bool(*b),
        Expr::Null => Value::Null,
        Expr::Var(name) => {
            if let Some(v) = locals.get(name) {
                v.clone()
            } else if let Some(v) = globals.get(name) {
                v.clone()
            } else if let Some(def) = funcs.get(name) {
                if def.params.is_empty() {
                    call(name, &[], globals, funcs)?
                } else {
                    return Err(format!("`{name}` requires arguments"));
                }
            } else {
                return Err(format!("undefined variable `{name}`"));
            }
        }
        Expr::Unary(op, x) => {
            let Flow::Value(v) = eval(x, locals, globals, funcs)? else {
                return Err("`return` outside function".into());
            };
            match (op.as_str(), v) {
                ("!", v) => Value::Bool(!truthy(&v)),
                ("-", Value::Num(n)) => Value::Num(-n),
                (o, _) => return Err(format!("bad unary `{o}`")),
            }
        }
        Expr::Binary(op, l, r) => {
            let Flow::Value(lv) = eval(l, locals, globals, funcs)? else {
                return Err("`return` outside function".into());
            };
            let Flow::Value(rv) = eval(r, locals, globals, funcs)? else {
                return Err("`return` outside function".into());
            };
            binop(op, &lv, &rv)?
        }
        Expr::Call(name, args) => {
            let mut vals = Vec::new();
            for a in args {
                let Flow::Value(v) = eval(a, locals, globals, funcs)? else {
                    return Err("`return` outside function".into());
                };
                vals.push(v);
            }
            call(name, &vals, globals, funcs)?
        }
    };
    Ok(Flow::Value(v))
}

pub fn call(
    name: &str,
    args: &[Value],
    globals: &mut Env,
    funcs: &BTreeMap<String, FuncDef>,
) -> Result<Value> {
    match name {
        "log" => {
            let s = args.iter().map(display).collect::<Vec<_>>().join(" ");
            eprintln!("[script] {s}");
            return Ok(Value::Null);
        }
        "str" => {
            return Ok(Value::Str(args.first().map(display).unwrap_or_default()));
        }
        "num" => match args.first() {
            Some(Value::Num(n)) => return Ok(Value::Num(*n)),
            Some(Value::Str(s)) => {
                return Ok(Value::Num(s.trim().parse().map_err(|_| format!("num: cannot parse `{s}`"))?))
            }
            Some(Value::Bool(b)) => return Ok(Value::Num(if *b { 1.0 } else { 0.0 })),
            _ => return Ok(Value::Num(0.0)),
        },
        "len" => match args.first() {
            Some(Value::Str(s)) => return Ok(Value::Num(s.chars().count() as f64)),
            _ => return Err("len: expects a string".into()),
        },
        "floor" | "ceil" | "round" => match args.first() {
            Some(Value::Num(n)) => {
                return Ok(Value::Num(match name {
                    "floor" => n.floor(),
                    "ceil" => n.ceil(),
                    _ => n.round(),
                }))
            }
            _ => return Err(format!("{name}: expects a number")),
        },
        _ => {}
    }
    let def = funcs
        .get(name)
        .ok_or_else(|| format!("unknown function `{name}`"))?
        .clone();
    if args.len() != def.params.len() {
        return Err(format!(
            "`{name}` expects {} args, got {}",
            def.params.len(),
            args.len()
        ));
    }
    let mut locals: BTreeMap<String, Value> = def
        .params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    match exec(&def.body, &mut locals, globals, funcs)? {
        Flow::Return(v) | Flow::Value(v) => Ok(v),
        Flow::Next => Ok(Value::Null),
    }
}

pub fn exec(
    stmts: &[Stmt],
    locals: &mut BTreeMap<String, Value>,
    globals: &mut Env,
    funcs: &BTreeMap<String, FuncDef>,
) -> Result<Flow> {
    for s in stmts {
        match s {
            Stmt::Let(n, e) => {
                let Flow::Value(v) = eval(e, locals, globals, funcs)? else {
                    return Err("`return` outside function".into());
                };
                locals.insert(n.clone(), v);
            }
            Stmt::Assign(n, e) => {
                let Flow::Value(v) = eval(e, locals, globals, funcs)? else {
                    return Err("`return` outside function".into());
                };
                if locals.contains_key(n) {
                    locals.insert(n.clone(), v);
                } else if globals.contains_key(n) {
                    globals.insert(n.clone(), v);
                } else {
                    return Err(format!("assignment to undefined variable `{n}`"));
                }
            }
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => {
                        let Flow::Value(v) = eval(e, locals, globals, funcs)? else {
                            return Err("`return` outside function".into());
                        };
                        v
                    }
                    None => Value::Null,
                };
                return Ok(Flow::Return(v));
            }
            Stmt::If(c, then, els) => {
                let Flow::Value(cv) = eval(c, locals, globals, funcs)? else {
                    return Err("`return` outside function".into());
                };
                if truthy(&cv) {
                    return exec(then, locals, globals, funcs);
                } else if !els.is_empty() {
                    return exec(els, locals, globals, funcs);
                }
            }
            Stmt::ExprStmt(e) => {
                eval(e, locals, globals, funcs)?;
            }
        }
    }
    Ok(Flow::Value(Value::Null))
}
