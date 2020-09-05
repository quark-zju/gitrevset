use crate::Error;
use crate::Result;
use gitdag::dag::Set;
use std::borrow::Cow;
use std::fmt;

/// A node in the parsed AST.
#[derive(Clone)]
pub enum Expr {
    /// A plain string name.
    Name(String),

    /// A function call.
    Fn(Cow<'static, str>, Vec<Expr>),

    /// An inlined Set.
    Inlined(Set),
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
            Expr::Inlined(set) => set.fmt(f)?,
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
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
            Expr::Fn(name, args) => {
                // Special case: hold the first argument of "apply" unchanged.
                if name == "apply" && args.len() > 1 {
                    for arg in &mut args[1..] {
                        arg.replace(from, to);
                    }
                } else {
                    for arg in args {
                        arg.replace(from, to);
                    }
                }
            }
            Expr::Inlined(_) => (),
        }
    }

    /// Parse AST from a string.
    pub fn parse(s: &str) -> Result<Self> {
        crate::parser::parse(s).map_err(|e| Error::ParseError(e.to_string()))
    }
}

/// Convert to `Expr` by parsing.
pub trait ParseToExpr {
    /// Convert to `Expr` by parsing.
    fn parse_to_expr(self) -> Result<Expr>;
}

impl ParseToExpr for &str {
    fn parse_to_expr(self) -> Result<Expr> {
        Expr::parse(self)
    }
}

impl ParseToExpr for Expr {
    fn parse_to_expr(self) -> Result<Expr> {
        Ok(self)
    }
}

impl From<&str> for Expr {
    fn from(s: &str) -> Expr {
        Expr::Name(s.to_string())
    }
}

impl From<Set> for Expr {
    fn from(s: Set) -> Expr {
        Expr::Inlined(s)
    }
}

/// Construct an AST statically. This can be useful to avoid escaping issues
/// parsing strings.
///
/// # Example
///
/// ```
/// # use gitrevset::ast;
/// let expr = ast!(union(draft(), desc("foo")));
/// assert_eq!(expr.to_string(), "union(draft(), desc(foo))");
/// ```
///
/// Use `{ ... }` to refer to local variables:
///
/// ```
/// # use gitrevset::ast;
/// let name = "origin/master";
/// let expr = ast!(ref({ name }));
/// assert_eq!(expr.to_string(), "ref(origin/master)");
/// let nested = ast!(parents({ expr }));
/// assert_eq!(nested.to_string(), "parents(ref(origin/master))");
/// ```
///
/// `Set` can also be referred:
///
/// ```
/// # fn main() -> gitrevset::Result<()> {
/// # #[cfg(feature = "testutil")]
/// # {
/// # use gitrevset::ast;
/// # use gitrevset::dag::DagAlgorithm;
/// # let mut repo = gitrevset::TestRepo::new();
/// # repo.drawdag("A--B");
/// let head = repo.revs(ast!(head()))?;
/// let set = repo.revs(ast!(parents({ head })))?;
///
/// // The above is similar to using raw `dag` APIs:
/// let set2 = {
///     let dag = repo.dag();
///     dag.parents(dag.heads(dag.all()?)?)?
/// };
/// # assert_eq!((set2.clone() - set.clone()).count()?, 0);
/// # assert_eq!((set.clone() - set2.clone()).count()?, 0);
/// # }
/// # Ok(())
/// # }
#[macro_export]
macro_rules! ast {
    ($v:literal) => { $crate::Expr::Name($v.to_string()) };
    ($fname:ident ( $($arg:tt $( ( $($subargs:tt)* ) )? ),* )) => {{
        let args = vec![ $(ast!($arg $( ( $($subargs)* ) )?),)* ];
        $crate::Expr::Fn(stringify!($fname).into(), args)
    }};
    {$v:expr} => { $crate::Expr::from($v) };
}
