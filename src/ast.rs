use crate::Error;
use crate::Result;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt;

/// A node in the parsed AST.
#[derive(Clone)]
pub enum Expr {
    /// A plain string name.
    Name(String),

    /// A function call.
    Fn(Cow<'static, str>, Vec<Expr>),
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Name(s) => f.write_str(&s)?,
            Expr::Fn(name, args) => {
                if args.is_empty() {
                    f.write_str(name)?;
                    f.write_str("()")?;
                } else {
                    let mut list = f.debug_tuple(&name);
                    for arg in args {
                        list.field(arg);
                    }
                    list.finish()?;
                }
            }
        }
        Ok(())
    }
}

impl Expr {
    /// Replace a name (ex. `$1`) to another name.
    /// Useful to support user-defined aliases.
    pub(crate) fn replace(&mut self, from: &str, to: &Expr) {
        match self {
            Expr::Name(s) => {
                if &s[..] == from {
                    *self = to.clone();
                }
            }
            Expr::Fn(_name, args) => {
                for arg in args {
                    arg.replace(from, to);
                }
            }
        }
    }
}

impl TryFrom<&str> for Expr {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self> {
        crate::parser::parse(s).map_err(|e| Error::ParseError(e.to_string()))
    }
}

/// Construct an AST statically.
#[macro_export]
macro_rules! ast {
    ($v:literal) => { $crate::Expr::Name($v.to_string()) };
    ($fname:ident ( $($arg:tt $( $subargs:tt )? ),* )) => {{
        let args = vec![ $(ast!($arg $( $subargs )?),)* ];
        $crate::Expr::Fn(stringify!($fname).into(), args)
    }};
}