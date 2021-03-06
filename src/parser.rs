use codemap::Spanned;
use lexer::Token;
use std::iter::Peekable;
use std::slice::Iter;
use self::ParseErrorKind::*;

type ParseIter<'a> = Peekable<Iter<'a, Spanned<Token>>>;
type Parse = Result<Node, ParseErrorKind>;

#[derive(Debug, Clone, PartialEq)]
pub enum Var {
  Single(String),
  Multi(Vec<Var>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Place {
  Single(Box<Node>),
  Multi(Vec<Place>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
  Block(Vec<Node>),
  Stmt(Box<Node>),
  Catch(Vec<Node>),
  Assn {
    lhs: Place,
    rhs: Box<Node>,
  },
  If {
    cond: Box<Node>,
    body: Vec<Node>,
    els: Option<Box<Node>>,
  },
  ElseIf {
    cond: Box<Node>,
    body: Vec<Node>,
  },
  Else {
    body: Vec<Node>,
  },
  For {
    decl: Var,
    expr: Box<Node>,
    body: Vec<Node>,
  },
  While {
    expr: Box<Node>,
    body: Vec<Node>,
  },
  Loop {
    body: Vec<Node>,
  },
  Return(Option<Box<Node>>),
  Break,
  Continue,
  Expr,
  Pass,
  Index {
    lhs: Box<Node>,
    rhs: Box<Node>,
  },

  Method {
    owner: Box<Node>,
    method: Box<Node>,
    args: Vec<Node>,
  },

  Func {
    params: Vec<String>,
    body: Vec<Node>,
  },

  Lambda {
    params: Vec<String>,
    expr: Box<Node>,
  },

  Call {
    func: Box<Node>,
    args: Vec<Node>,
  },

  BinExpr {
    lhs: Box<Node>,
    op: Token,
    rhs: Box<Node>,
  },

  UnExpr {
    val: Box<Node>,
    op: Token,
  },

  // Literals
  Null,
  Bool(bool),
  Float(f64),
  Int(i64),
  Str(String),
  Name(String),
  Table,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
  Right(u32),
  Left(u32),
  None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseErrorKind {
  UnexpectedToken(Token),
  UnexpectedEOF,
  UnknownBinaryOperator,
  UnknownUnaryOperator,
  UnusedPlaces,
}

// Return true if the next token in `it` is `kind`
fn peek_token(it: &mut ParseIter, kind: Token) -> bool {
  if let Some(&tok) = it.peek() {
    tok.node == kind
  } else {
    false
  }
}

// Return true if the next token in `it` is `kind` *and* consume the token
fn use_token(it: &mut ParseIter, kind: Token) -> bool {
  if let Some(&tok) = it.peek() {
    if tok.node == kind {
      it.next();
    }
    tok.node == kind
  } else {
    false
  }
}

// Panic if the next token in `it` is *not* `kind`
fn require_token(it: &mut ParseIter, kind: Token) -> Result<(), ParseErrorKind> {
  if let Some(&tok) = it.peek() {
    if tok.node == kind {
      it.next();
      return Ok(());
    }

    return Err(UnexpectedToken(tok.node.clone()));
  }

  return Err(UnexpectedEOF);
}

fn op_precedence(op: &Token) -> Op {
  match *op {
    Token::Add | Token::Sub => Op::Left(10),
    Token::Div | Token::Mul => Op::Left(20),
    Token::Car => Op::Right(30),
    _ => Op::None,
  }
}

fn parse_ml_expr(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Func => {
        it.next();
        require_token(it, Token::Pal)?;
        let params = parse_fn_params(it)?;
        require_token(it, Token::Par)?;
        let body = parse_block(it)?;
        Ok(Node::Func {
          params: params,
          body: body,
        })
      }
      Token::Catch => {
        it.next();
        let block = parse_block(it)?;
        Ok(Node::Catch(block))
      }
      _ => parse_il_expr(it),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_il_expr(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Or => {
        it.next();
        let params = parse_fn_params(it)?;
        require_token(it, Token::Or)?;
        let expr = parse_il_expr(it)?;
        Ok(Node::Lambda {
          params: params,
          expr: Box::new(expr),
        })
      }
      _ => parse_bin_expr(it),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_bin_expr(it: &mut ParseIter) -> Parse {
  let mut expr = parse_un_expr(it)?;

  // prevents this from breaking the LHS until we know we made it
  // otherwise, things like (2 + 3) * 4 get restructured into 2 + (3 * 4)
  let mut break_left = false;

  while let Some(&tok) = it.peek() {
    let prec = op_precedence(&tok.node);

    if let Op::None = prec {
      break;
    }

    it.next();

    let rhs = parse_un_expr(it)?;

    expr = match (break_left, expr.clone()) {
      (
        true,
        Node::BinExpr {
          lhs: cur_lhs,
          op: cur_op,
          rhs: cur_rhs,
        },
      ) => {
        let cur_prec = op_precedence(&cur_op);
        match (cur_prec, prec) {
          // these should never happen
          (_, Op::None) => break,
          (Op::None, _) => break,

          // left-to-right
          // there has to be a better way to handle this, no?
          (Op::Left(n), Op::Left(m)) if n >= m => Node::BinExpr {
            lhs: Box::new(expr),
            op: tok.node.clone(),
            rhs: Box::new(rhs),
          },
          (Op::Right(n), Op::Right(m)) if n > m => Node::BinExpr {
            lhs: Box::new(expr),
            op: tok.node.clone(),
            rhs: Box::new(rhs),
          },
          (Op::Right(n), Op::Left(m)) if n >= m => Node::BinExpr {
            lhs: Box::new(expr),
            op: tok.node.clone(),
            rhs: Box::new(rhs),
          },
          (Op::Left(n), Op::Right(m)) if n >= m => Node::BinExpr {
            lhs: Box::new(expr),
            op: tok.node.clone(),
            rhs: Box::new(rhs),
          },

          // right-to-left
          _ => Node::BinExpr {
            lhs: cur_lhs,
            op: cur_op,
            rhs: Box::new(Node::BinExpr {
              lhs: cur_rhs,
              op: tok.node.clone(),
              rhs: Box::new(rhs),
            }),
          },
        }
      }
      _ => Node::BinExpr {
        lhs: Box::new(expr),
        op: tok.node.clone(),
        rhs: Box::new(rhs),
      },
    };

    break_left = true;
  }

  Ok(expr)
}

fn parse_un_expr(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Sub | Token::Not | Token::Neg => {
        it.next();
        let val = parse_un_expr(it)?;
        Ok(Node::UnExpr {
          op: tok.node.clone(),
          val: Box::new(val),
        })
      }
      _ => parse_simple(it),
    };
  }

  Err(UnexpectedEOF)
}

/* unused, here for reference
fn parse_index(it: &mut ParseIter) -> Parse {
  if let Some(&c) = it.peek() {
    return match c.node {
      Token::Sql => {
        it.next();
        let idx = parse_bin_expr(it)?;
        require_token(it, Token::Sqr)?;
        Ok(idx)
      }
      Token::Dot => {
        it.next();
        parse_name_as_str(it)
      }
      _ => {
        Err(UnexpectedToken(c.node.clone()))
      }
    };
  }

  Err(UnexpectedEOF)
}
*/

fn parse_fn_params(it: &mut ParseIter) -> Result<Vec<String>, ParseErrorKind> {
  let mut params: Vec<String> = Vec::new();
  while let Some(&tok) = it.peek() {
    match tok.node {
      Token::Name(ref x) => {
        it.next();
        params.push(x.to_string());
        if !use_token(it, Token::Com) {
          break;
        }
      }
      _ => break,
    }
  }

  Ok(params)
}

fn parse_fn_args(it: &mut ParseIter) -> Result<Vec<Node>, ParseErrorKind> {
  let mut args = Vec::new();
  require_token(it, Token::Pal)?;
  while !peek_token(it, Token::Par) {
    let arg = parse_il_expr(it)?;
    args.push(arg);
    if !use_token(it, Token::Com) {
      break;
    }
  }
  require_token(it, Token::Par)?;
  Ok(args)
}

fn parse_simple(it: &mut ParseIter) -> Parse {
  let mut atom = parse_atom(it)?;
  while let Some(&tok) = it.peek() {
    match tok.node {
      Token::Col => {
        it.next();
        let method = parse_name_as_str(it)?;
        let args = parse_fn_args(it)?;
        atom = Node::Method {
          owner: Box::new(atom),
          method: Box::new(method),
          args: args,
        };
      }

      Token::Pal => {
        let args = parse_fn_args(it)?;
        atom = Node::Call {
          func: Box::new(atom),
          args: args,
        };
      }

      Token::Sql => {
        it.next();
        let idx = parse_bin_expr(it)?;
        require_token(it, Token::Sqr)?;
        atom = Node::Index {
          lhs: Box::new(atom),
          rhs: Box::new(idx),
        };
      }

      Token::Dot => {
        it.next();
        let idx = parse_name_as_str(it)?;
        atom = Node::Index {
          lhs: Box::new(atom),
          rhs: Box::new(idx),
        };
      }

      _ => break,
    }
  }

  Ok(atom)
}

fn parse_atom(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Pal => {
        it.next();
        let out = parse_bin_expr(it)?;
        require_token(it, Token::Par)?;
        Ok(out)
      }
      _ => parse_quark(it),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_name_as_str(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Name(ref x) => {
        it.next();
        Ok(Node::Str(x.clone()))
      }
      ref x => Err(UnexpectedToken(x.clone())),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_name(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Name(ref x) => {
        it.next();
        Ok(Node::Name(x.clone()))
      }
      ref x => Err(UnexpectedToken(x.clone())),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_quark(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Null => {
        it.next();
        Ok(Node::Null)
      }
      Token::Bool(x) => {
        it.next();
        Ok(Node::Bool(x))
      }
      Token::Float(x) => {
        it.next();
        Ok(Node::Float(x))
      }
      Token::Int(x) => {
        it.next();
        Ok(Node::Int(x))
      }
      Token::Str(ref x) => {
        it.next();
        Ok(Node::Str(x.clone()))
      }
      Token::Name(ref x) => {
        it.next();
        Ok(Node::Name(x.clone()))
      }
      Token::Table => {
        it.next();
        Ok(Node::Table)
      }
      ref x => Err(UnexpectedToken(x.clone())),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_decl(it: &mut ParseIter) -> Result<Var, ParseErrorKind> {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Sql => {
        it.next();
        let mut pieces: Vec<Var> = Vec::new();
        loop {
          let new_piece = parse_decl(it)?;
          pieces.push(new_piece);
          if !use_token(it, Token::Com) {
            break;
          }
        }
        require_token(it, Token::Sqr)?;
        Ok(Var::Multi(pieces))
      }
      Token::Name(ref x) => {
        it.next();
        Ok(Var::Single(x.clone()))
      }
      ref x => Err(UnexpectedToken(x.clone())),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_place(it: &mut ParseIter) -> Result<Place, ParseErrorKind> {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Sql => {
        it.next();
        let mut pieces: Vec<Place> = Vec::new();
        loop {
          let new_piece = parse_place(it)?;
          pieces.push(new_piece);
          if !use_token(it, Token::Com) {
            break;
          }
        }
        require_token(it, Token::Sqr)?;
        Ok(Place::Multi(pieces))
      }

      _ => {
        let node = parse_il_expr(it)?;
        Ok(Place::Single(Box::new(node)))
      }
    };
  }

  Err(UnexpectedEOF)
}

fn parse_assn(it: &mut ParseIter) -> Parse {
  let place = parse_place(it)?;

  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Ass => {
        it.next();
        let rhs = parse_ml_expr(it)?;
        Ok(Node::Assn {
          lhs: place,
          rhs: Box::new(rhs),
        })
      }

      _ => match place {
        Place::Single(bx) => Ok(Node::Stmt(bx)),
        Place::Multi(_) => Err(UnusedPlaces),
      },
    };
  }

  Err(UnexpectedEOF)
}

fn parse_stmt(it: &mut ParseIter) -> Parse {
  if let Some(&tok) = it.peek() {
    return match tok.node {
      Token::Break => {
        it.next();
        Ok(Node::Break)
      }

      Token::Continue => {
        it.next();
        Ok(Node::Continue)
      }

      Token::If => {
        it.next();
        let cond = parse_bin_expr(it)?;
        let body = parse_block(it)?;
        Ok(Node::If {
          cond: Box::new(cond),
          body: body,
          els: None,
        })
      }

      Token::Else => {
        it.next();
        if use_token(it, Token::If) {
          let cond = parse_bin_expr(it)?;
          let body = parse_block(it)?;
          Ok(Node::ElseIf {
            cond: Box::new(cond),
            body: body,
          })
        } else {
          let body = parse_block(it)?;
          Ok(Node::Else { body: body })
        }
      }

      Token::For => {
        it.next();
        let decl = parse_decl(it)?;
        require_token(it, Token::In)?;
        let expr = parse_il_expr(it)?;
        let body = parse_block(it)?;
        Ok(Node::For {
          decl: decl,
          expr: Box::new(expr),
          body: body,
        })
      }

      Token::While => {
        it.next();
        let expr = parse_il_expr(it)?;
        let body = parse_block(it)?;
        Ok(Node::While {
          expr: Box::new(expr),
          body: body,
        })
      }

      Token::Loop => {
        it.next();
        let body = parse_block(it)?;
        Ok(Node::Loop { body: body })
      }

      Token::Return => {
        it.next();
        let val = if peek_token(it, Token::End) {
          None
        } else {
          let val = parse_ml_expr(it)?;
          Some(Box::new(val))
        };
        Ok(Node::Return(val))
      }

      Token::Pass => {
        it.next();
        Ok(Node::Pass)
      }

      Token::Func | Token::Catch => parse_ml_expr(it).map(|expr| Node::Stmt(Box::new(expr))),

      _ => parse_assn(it),
    };
  }

  Err(UnexpectedEOF)
}

fn parse_block(it: &mut ParseIter) -> Result<Vec<Node>, ParseErrorKind> {
  let mut nodes: Vec<Node> = vec![];

  require_token(it, Token::Enter)?;

  while !peek_token(it, Token::Exit) {
    let stmt = parse_stmt(it)?;
    nodes.push(stmt);
    require_token(it, Token::End)?;
  }

  require_token(it, Token::Exit)?;

  Ok(nodes)
}

pub fn parse(tokens: Vec<Spanned<Token>>) -> Parse {
  let mut it: ParseIter = tokens.iter().peekable();
  let mut nodes: Vec<Node> = vec![];

  while !peek_token(&mut it, Token::EOF) {
    let stmt = parse_stmt(&mut it)?;
    nodes.push(stmt);
    require_token(&mut it, Token::End)?;
  }

  Ok(Node::Block(nodes))
}

#[cfg(test)]
#[path = "./tests/parser.rs"]
mod tests;
