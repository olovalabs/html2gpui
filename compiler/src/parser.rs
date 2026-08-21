use crate::ast::{Expr, FuncDef, IrScript, Stmt};
use crate::tokenizer::Tok;
use crate::types::Result;

pub struct Parser {
    pub toks: Vec<Tok>,
    pub pos: usize,
}

impl Parser {
    pub fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    pub fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    pub fn eat_newlines(&mut self) {
        while self.peek() == Some(&Tok::Newline) {
            self.pos += 1;
        }
    }
    pub fn expect_punct(&mut self, p: &str) -> Result<()> {
        match self.next() {
            Some(Tok::Punct(q)) if q == p => Ok(()),
            other => Err(format!("expected `{p}`, got {other:?}")),
        }
    }

    pub fn script(&mut self) -> Result<IrScript> {
        let mut out = IrScript::default();
        loop {
            self.eat_newlines();
            match self.peek() {
                None => break,
                Some(Tok::Ident(w)) if w == "let" => {
                    self.next();
                    let name = match self.next() {
                        Some(Tok::Ident(n)) => n,
                        other => return Err(format!("let: expected name, got {other:?}")),
                    };
                    self.expect_punct("=")?;
                    let e = self.expr()?;
                    out.vars.push((name, e));
                    self.stmt_end()?;
                }
                Some(Tok::Ident(w)) if w == "function" => {
                    self.next();
                    let name = match self.next() {
                        Some(Tok::Ident(n)) => n,
                        other => return Err(format!("function: expected name, got {other:?}")),
                    };
                    self.expect_punct("(")?;
                    let mut params = Vec::new();
                    loop {
                        self.eat_newlines();
                        match self.peek() {
                            Some(Tok::Punct(")")) => {
                                self.next();
                                break;
                            }
                            Some(Tok::Ident(_)) => {
                                if let Some(Tok::Ident(p)) = self.next() {
                                    params.push(p);
                                }
                                self.eat_newlines();
                                if self.peek() == Some(&Tok::Punct(",")) {
                                    self.next();
                                }
                            }
                            other => return Err(format!("params: unexpected {other:?}")),
                        }
                    } 
                    let body = self.block()?;
                    out.funcs.insert(name, FuncDef { params, body });
                }
                other => return Err(format!("top level: unexpected {other:?} (use `let` or `function`)")),
            }
        }
        Ok(out)
    }

    pub fn stmt_end(&mut self) -> Result<()> {
        match self.peek() {
            Some(Tok::Punct(";")) | Some(Tok::Newline) | Some(Tok::Punct("}")) | None => {
                if self.peek() == Some(&Tok::Punct(";")) {
                    self.next();
                }
                self.eat_newlines();
                Ok(())
            }
            other => Err(format!("expected end of statement, got {other:?}")),
        }
    }

    pub fn block(&mut self) -> Result<Vec<Stmt>> {
        self.eat_newlines();
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        loop {
            self.eat_newlines();
            match self.peek() {
                None => return Err("unexpected end of file in block".into()),
                Some(Tok::Punct("}")) => {
                    self.next();
                    break;
                }
                _ => stmts.push(self.stmt()?),
            }
        }
        Ok(stmts)
    }

    pub fn stmt(&mut self) -> Result<Stmt> {
        match self.peek().cloned() {
            Some(Tok::Ident(w)) if w == "let" => {
                self.next();
                let name = match self.next() {
                    Some(Tok::Ident(n)) => n,
                    other => return Err(format!("let: expected name, got {other:?}")),
                };
                self.expect_punct("=")?;
                let e = self.expr()?;
                self.stmt_end()?;
                Ok(Stmt::Let(name, e))
            }
            Some(Tok::Ident(w)) if w == "return" => {
                self.next();
                let ends = matches!(
                    self.peek(),
                    Some(Tok::Newline) | Some(Tok::Punct(";")) | Some(Tok::Punct("}")) | None
                );
                if ends {
                    self.stmt_end()?;
                    Ok(Stmt::Return(None))
                } else {
                    let e = self.expr()?;
                    self.stmt_end()?;
                    Ok(Stmt::Return(Some(e)))
                }
            }
            Some(Tok::Ident(w)) if w == "if" => {
                self.next();
                self.expect_punct("(")?;
                let cond = self.expr()?;
                self.expect_punct(")")?;
                let then = self.block()?;
                self.eat_newlines();
                let els = if self.peek() == Some(&Tok::Ident("else".into())) {
                    self.next();
                    self.eat_newlines();
                    if self.peek() == Some(&Tok::Ident("if".into())) {
                        vec![self.stmt()?]
                    } else {
                        self.block()?
                    }
                } else {
                    Vec::new() 
                };
                Ok(Stmt::If(cond, then, els))
            }
            _ => {
                let e = self.expr_full()?;
                self.stmt_end()?;
                Ok(e)
            }
        }
    }

    pub fn expr_full(&mut self) -> Result<Stmt> {
        if let Some(Tok::Ident(name)) = self.peek().cloned() {
            let nxt = self.toks.get(self.pos + 1);
            if nxt == Some(&Tok::Punct("=")) && self.toks.get(self.pos + 2) != Some(&Tok::Punct("="))
            {
                self.next();
                self.next();
                let e = self.expr()?;
                return Ok(Stmt::Assign(name, e));
            }
            let postfix_delta: Option<f64> = match nxt {
                Some(Tok::Punct("++")) => Some(1.0),
                Some(Tok::Punct("--")) => Some(-1.0),
                _ => None,
            };
            if let Some(delta) = postfix_delta {
                self.next();
                self.next();
                return Ok(Stmt::Assign(
                    name.clone(),
                    Expr::Binary(
                        "+".to_string(),
                        Box::new(Expr::Var(name)),
                        Box::new(Expr::Num(delta)),
                    ),
                ));
            }
        }
        Ok(Stmt::ExprStmt(self.expr()?))
    }

    pub fn expr(&mut self) -> Result<Expr> {
        self.or_expr()
    }
    fn or_expr(&mut self) -> Result<Expr> {
        let mut l = self.and_expr()?;
        while self.peek() == Some(&Tok::Punct("||")) {
            self.next();
            l = Expr::Binary("||".into(), Box::new(l), Box::new(self.and_expr()?));
        }
        Ok(l)
    }
    fn and_expr(&mut self) -> Result<Expr> {
        let mut l = self.eq_expr()?;
        while self.peek() == Some(&Tok::Punct("&&")) {
            self.next();
            l = Expr::Binary("&&".into(), Box::new(l), Box::new(self.eq_expr()?));
        }
        Ok(l)
    }
    fn eq_expr(&mut self) -> Result<Expr> {
        let mut l = self.rel_expr()?;
        while matches!(self.peek(), Some(Tok::Punct("==")) | Some(Tok::Punct("!="))) {
            let op = if let Some(Tok::Punct(p)) = self.next() {
                p.to_string()
            } else {
                unreachable!()
            };
            l = Expr::Binary(op, Box::new(l), Box::new(self.rel_expr()?));
        }
        Ok(l)
    }
    fn rel_expr(&mut self) -> Result<Expr> {
        let mut l = self.add_expr()?;
        while matches!(
            self.peek(),
            Some(Tok::Punct("<")) | Some(Tok::Punct(">")) | Some(Tok::Punct("<=")) | Some(Tok::Punct(">="))
        ) {
            let op = if let Some(Tok::Punct(p)) = self.next() {
                p.to_string()
            } else {
                unreachable!()
            };
            l = Expr::Binary(op, Box::new(l), Box::new(self.add_expr()?));
        }
        Ok(l)
    }
    fn add_expr(&mut self) -> Result<Expr> {
        let mut l = self.mul_expr()?;
        while matches!(self.peek(), Some(Tok::Punct("+")) | Some(Tok::Punct("-"))) {
            let op = if let Some(Tok::Punct(p)) = self.next() {
                p.to_string()
            } else {
                unreachable!()
            };
            l = Expr::Binary(op, Box::new(l), Box::new(self.mul_expr()?));
        }
        Ok(l)
    }
    fn mul_expr(&mut self) -> Result<Expr> {
        let mut l = self.unary()?;
        while matches!(
            self.peek(),
            Some(Tok::Punct("*")) | Some(Tok::Punct("/")) | Some(Tok::Punct("%"))
        ) {
            let op = if let Some(Tok::Punct(p)) = self.next() {
                p.to_string()
            } else {
                unreachable!()
            };
            l = Expr::Binary(op, Box::new(l), Box::new(self.unary()?));
        }
        Ok(l)
    }
    fn unary(&mut self) -> Result<Expr> {
        if matches!(self.peek(), Some(Tok::Punct("!")) | Some(Tok::Punct("-"))) {
            let op = if let Some(Tok::Punct(p)) = self.next() {
                p.to_string()
            } else {
                unreachable!()
            };
            return Ok(Expr::Unary(op, Box::new(self.unary()?)));
        }
        self.primary()
    }
    fn primary(&mut self) -> Result<Expr> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Num(n)),
            Some(Tok::Str(s)) => Ok(Expr::Str(s)),
            Some(Tok::Ident(w)) if w == "true" => Ok(Expr::Bool(true)),
            Some(Tok::Ident(w)) if w == "false" => Ok(Expr::Bool(false)),
            Some(Tok::Ident(w)) if w == "null" => Ok(Expr::Null),
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::Punct("(")) {
                    self.next();
                    let mut args = Vec::new();
                    loop {
                        self.eat_newlines();
                        if self.peek() == Some(&Tok::Punct(")")) {
                            self.next();
                            break;
                        }
                        args.push(self.expr()?);
                        self.eat_newlines();
                        if self.peek() == Some(&Tok::Punct(",")) {
                            self.next();
                        }
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Some(Tok::Punct("(")) => {
                let e = self.expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            other => Err(format!("unexpected token {other:?} in expression")),
        }
    }
}
