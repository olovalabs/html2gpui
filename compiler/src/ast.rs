use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct IrScript {
    pub vars: Vec<(String, Expr)>,
    pub funcs: BTreeMap<String, FuncDef>,
}

#[derive(Clone, Debug)]
pub struct FuncDef {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    Return(Option<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    ExprStmt(Expr),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Var(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
}

pub type Env = BTreeMap<String, Value>;

pub enum Flow {
    Next,
    Value(Value),
    Return(Value),
}
