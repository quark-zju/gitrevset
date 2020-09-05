//! A domain-specific-language to select commits in a git repo. Similar to
//! [Mercurial's revset](https://www.mercurial-scm.org/repo/hg/help/revsets).
//!
//! ## Language Specification
//!
//! Specifying a commit:
//! - Reference names like `master`, `release-foo`, or `origin/master`.
//! - Hex commit hashes or hash prefixes.
//! - A dot `.`, or the at sign `@` refers to `HEAD`.
//!
//! Operators:
//! - `x + y`, `x | y`, `x or y`, `union(x, y)`: Union of `x` and `y` (1).
//! - `x & y`, `x and y`, `intersection(x, y)`: Intersection of `x` and `y`.
//! - `x - y`, `difference(x, y)`: Commits in `x` but not `y`.
//! - `!x`, `not x`, `negate(x)`: Commits not in `x`.
//! - `::x`, `ancestors(x)`: Ancestors of `x`, including `x`.
//! - `x::`, `descendants(x)`: Descendants of `x`, including `x`.
//! - `x^`, `parents(x)`: Parents of `x` (2).
//! - `x % y`, `only(x, y)`: Reachable from `x`, not `y`, or `::x - ::y`.
//! - `x:y`, `range(x, y)`: A DAG range, descendants of `x` and ancestors of
//!   `y`, or `x:: & ::y` (3).
//!
//! Functions:
//! - `children(x)`: Commits with at least one parent in the `x` set.
//! - `heads(x)`: Heads of a set, `x - parents(x)`.
//! - `roots(x)`: Roots of a set, `x - children(x)`.
//! - `gca(x, y, ...)`, `ancestor(x, y, ...)`: Heads of common ancestors (4).
//! - `first(x, ...)`: First item in `x`, or `first(...)` if `x` is empty.
//! - `last(x)`: Last item in `x`, or empty.
//! - `head()`: Visible heads (references).
//! - `all()`: Visible commits, aka. `::head()`.
//! - `publichead()`: Heads referred by remotes, `ref("remotes/**")`.
//! - `drafthead()`: Heads not referred by remotes, `head() - publichead()`.
//! - `public()`: Commits reachable from `publichead()`, `::publichead()`.
//! - `draft()`: Commits only reachable from draft heads, `all() - public()`.
//! - `author(name)`: Filter by author name or email.
//! - `committer(name)`: Filter by committer name or email.
//! - `date(date)`: Filter by author date.
//! - `committerdate(date)`: Filter by committer date.
//! - `desc(text)`: Filter by commit message.
//! - `predecessors(x)`: Previous versions of `x`, including `x`.
//! - `successors(x)`: Newer versions of `x`, including `x`.
//! - `obsolete()`: Commits with at least one newer versions.
//! - `id(hexhash)`: Resolve a commit explicitly by a hex hash string.
//! - `ref()`: All references.
//! - `ref(name)`: Resolve commits by a reference name or glob.
//! - `tag()`: All tags.
//! - `tag(name)`: Resolve commits by a tag name or glob.
//! - `empty()`: Empty set.
//! - `present(set)`: Empty set on "unresolved name" error. Otherwise just `set`.
//! - `apply(expr, $1, $2, ...)`: Replace `$1`, `$2` in `expr` with evaluated
//!    sets. Then evaluate `expr`. Useful to avoid evaluate same sets multiple
//!    times.
//!
//! Differences from Mercurial:
//! 1. `x + y` does not make sure `y` comes after `x`. For example,
//!    `first(x + y)` might be `first(x)` or `first(y)`. In Mercurial,
//!    `first(x + y)` would be `first(x)`.
//! 2. `x^` selects all parents, not just first parents.
//! 3. `x:y` selects DAG range `x` to `y`, not revision number range.
//!    `x::y` is invalid syntax, for easier parsing.
//! 4. `ancestor(x, y)` can return multiple commits for criss-cross merges.
//!
//! ## Quick Start
//!
//! First, construct a [`gitrevset::Repo`](struct.Repo.html):
//!
//! ```
//! # fn main() -> gitrevset::Result<()> {
//! # #[cfg(feature = "testutil")]
//! # {
//! # use gitrevset::git2;
//! # let repo = gitrevset::TestRepo::new();
//! # repo.set_env();
//! # let path = repo.git_repo().path();
//! // Open from the current directory.
//! let repo = gitrevset::Repo::open_from_env()?;
//!
//! // Open from a libgit2 repository.
//! let git_repo = git2::Repository::open(path)?;
//! let repo = gitrevset::Repo::open_from_repo(Box::new(git_repo))?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! Then, use [`revs`](struct.Repo.html#method.revs) or
//! [`anyrevs`](struct.Repo.html#method.anyrevs) to perform queries.
//!
//! ```
//! # fn main() -> gitrevset::Result<()> {
//! # #[cfg(feature = "testutil")]
//! # {
//! # let mut repo = gitrevset::TestRepo::new();
//! # repo.drawdag("A");
//! # repo.add_ref("refs/heads/master", repo.query_single_oid("A"));
//! # repo.git_repo().config().unwrap().set_str("revsetalias.foo", "parents($1)").unwrap();
//! let set = repo.revs("(draft() & ::.)^ + .")?;
//!
//! // With user-defined aliases considered.
//! // ex. git config revsetalias.foo "parents($1)"
//! let set = repo.anyrevs("foo(.)")?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! Finally, use [`to_oids`](trait.SetExt.html#tymethod.to_oids) to extract Git
//! object IDs from the resulting set:
//!
//! ```
//! # fn main() -> gitrevset::Result<()> {
//! # #[cfg(feature = "testutil")]
//! # {
//! # let repo = gitrevset::TestRepo::new();
//! # let set = repo.revs("all()")?;
//! use gitrevset::SetExt;
//! for oid in set.to_oids()? {
//!     dbg!(oid?);
//! }
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! To parse the revset expression at compile time, to avoid issues about
//! string escaping or injection, use the [`ast!`](macro.ast.html) macro.
//!
//! ## Note on Commit Graph Index
//!
//! `gitrevset` takes advantage of the commit graph index from the
//! [EdenSCM](https://github.com/facebookexperimental/eden) project that is
//! designed to support very large repositories. The index is built on demand
//! under the `.git/dag` directory during the construction of the
//! [`Repo`](struct.Repo.html) struct.
//!
//! The index is highly optimized for ancestors-related queries.  For example,
//! `gca(x, y)`, `x & ::y`, `x % y` usually complete under 1 millisecond
//! regardless of the distance between `x` and `y`.
//!
//! The index is not optimized for many visible heads. Having too many
//! references might have a visible performance penalty on
//! [`Repo`](struct.Repo.html) construction.
//!
//! The index can be accessed by [`repo.dag()`](struct.Repo.html#method.dag)
//! and the re-exported `dag` crate.

#![allow(dead_code)]
#![deny(missing_docs)]

/// Extended methods on types defined in other crates.
pub mod ext;

mod ast;
mod error;
mod eval;
mod mutation;
mod parser;
mod repo;

#[cfg(any(test, feature = "testutil"))]
mod testrepo;

#[cfg(feature = "testutil")]
pub use testrepo::TestRepo;

#[cfg(test)]
mod tests;

pub use error::Error;
pub use gitdag::dag;
pub use gitdag::git2;

/// `Result` type used by `gitrevset`.
pub type Result<T> = std::result::Result<T, Error>;

pub use ast::Expr;
pub use eval::Context as EvalContext;
pub use ext::SetExt;
pub use repo::Repo;
