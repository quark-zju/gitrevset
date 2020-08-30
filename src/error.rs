use gitdag::dag::Vertex;
use std::convert::Infallible;
use thiserror::Error;

/// Error type used by `gitrevset`.
#[derive(Error, Debug)]
pub enum Error {
    /// Error caused by the commit graph indexing layer.
    #[error(transparent)]
    Dag(#[from] gitdag::dag::Error),

    /// Error caused by libgit2.
    #[error(transparent)]
    Git2(#[from] gitdag::git2::Error),

    /// A short commit hash can be resolved to multiple commits.
    #[error("ambiguous prefix: {0:?}")]
    AmbiguousPrefix(Vec<Vertex>),

    /// A name cannot be resolved.
    #[error("name {0:?} cannot be resolved")]
    UnresolvedName(String),

    /// A function call with wrong number of arguments.
    #[error("function {0} requires {1} arguments, but got {2} arguments")]
    MismatchedArguments(String, usize, usize),

    /// String is expected in the AST but got something different.
    #[error("expect string, got {0}")]
    ExpectString(String),

    /// An expression cannot be parsed into an AST.
    #[error("{0}")]
    ParseError(String),
}

impl From<Infallible> for Error {
    fn from(_e: Infallible) -> Self {
        unreachable!()
    }
}
