use std::collections::BTreeMap;
use crate::ast::IrScript;

pub type Result<T> = std::result::Result<T, String>;

pub struct Compiled {
    pub code: String,
    pub warnings: Vec<String>,
}

#[derive(Default, Clone)]
pub struct Ctx {
    pub comps: BTreeMap<String, String>,
    pub tag_styles: BTreeMap<String, Vec<(String, String)>>,
    pub class_styles: BTreeMap<String, Vec<(String, String)>>,
}

#[derive(Clone, Debug)]
pub struct IrDoc {
    pub entry: String,
    pub comps: BTreeMap<String, IrElem>,
    pub script: IrScript,
}

#[derive(Clone, Debug)]
pub struct IrElem {
    pub tag: String,
    pub decls: Vec<(String, String)>,
    pub events: Vec<(String, String)>,
    pub cond: Option<String>,
    pub src: Option<String>,
    pub children: Vec<IrChild>,
}

#[derive(Clone, Debug)]
pub enum IrChild {
    Div(IrElem),
    Comp(String, Vec<(String, String)>),
    Text(String),
    Tpl(Vec<TplPart>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TplPart {
    Lit(String),
    Var(String),
}

pub type Child = IrChild;
