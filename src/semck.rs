use parser::Node;
use parser::Place;

type Check = Result<(), CheckErrorKind>;

#[derive(Debug, Clone, PartialEq)]
pub enum CheckErrorKind {
  NotInLoop,
  MissingIf,
  NotPlace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemChecker {
  in_loop: bool,
  has_if: bool,
}

impl SemChecker {
  pub fn new() -> SemChecker {
    SemChecker {
      in_loop: false,
      has_if: false,
    }
  }

  pub fn check(&mut self, node: &mut Node) -> Check {
    println!("checking: {:?}", node);
    match *node {
      Node::Stmt(ref mut bx) => {
        self.check(bx)?;
      }

      Node::Block(ref mut ls) | Node::Catch(ref mut ls) => for mut n in ls {
        self.check(&mut n)?;
      },

      Node::Loop { ref mut body } => {
        self.in_loop = true;
        for mut n in body {
          self.check(&mut n)?;
        }
        self.in_loop = false;
      }

      Node::While {
        ref mut body,
        expr: _,
      } => {
        self.in_loop = true;
        for mut n in body {
          self.check(&mut n)?;
        }
        self.in_loop = false;
      }

      Node::For {
        ref mut body,
        decl: _,
        expr: _,
      } => {
        self.in_loop = true;
        for mut n in body {
          self.check(&mut n)?;
        }
        self.in_loop = false;
      }

      Node::Break | Node::Continue => {
        if !self.in_loop {
          return Err(CheckErrorKind::NotInLoop);
        }
      }

      Node::Assn { rhs: _, ref lhs } => {
        self.check_place(lhs)?;
      }

      // TODO add if-elif-else checks
      _ => {}
    }

    Ok(())
  }

  fn check_place(&self, place: &Place) -> Check {
    match *place {
      Place::Single(ref node) => {
        self.is_place(node)?;
      }
      Place::Multi(ref places) => {
        let mut valid = true;
        for pl in places {
          self.check_place(&pl)?;
        }
      }
    };
    Ok(())
  }

  fn is_place(&self, node: &Node) -> Check {
    match *node {
      Node::Name(_) | Node::Index { lhs: _, rhs: _ } => Ok(()),
      _ => Err(CheckErrorKind::NotPlace),
    }
  }
}
