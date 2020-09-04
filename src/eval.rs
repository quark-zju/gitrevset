use crate::ast::Expr;
use crate::repo::Repo;
use crate::Error;
use crate::Result;
use dag::ops::DagAlgorithm;
use dag::ops::PrefixLookup;
use dag::Set;
use dag::Vertex;
use gitdag::dag;
use hgtime::HgTime;
use gitdag::git2;
use globset::Glob;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;

type EvalFn = Box<dyn Fn(&str, &Repo, &[Expr], &Context) -> Result<Set>>;

/// Extra context for `eval`. It can define customized aliases.
#[derive(Default)]
pub struct Context {
    /// Extra pre-calculated sets. For example, if "foo" is defined here,
    /// "foo + x" will use the defined "foo" set.
    pub names: HashMap<String, Set>,

    /// Extra functions. For example, if "foo" is defined here, "foo(x)" will
    /// use the "foo" function.
    pub fns: HashMap<String, EvalFn>,
}

/// Evaluate an AST. Return the resulting set.
/// `context` can be used to define customized names or functions.
pub fn eval(repo: &Repo, expr: &Expr, context: &Context) -> Result<Set> {
    match expr {
        Expr::Name(name) => lookup(repo, name, context),
        Expr::Fn(name, args) => {
            let func = get_function(name, context)?;
            func(name, repo, args, context)
        }
    }
}

/// Resolve a name.
fn lookup(repo: &Repo, name: &str, context: &Context) -> Result<Set> {
    // User alias.
    if let Some(set) = context.names.get(name) {
        return Ok(set.clone());
    }

    let args = [Expr::Name(name.to_string())];

    // Resolve references.
    if let Ok(set) = r#ref("lookup", repo, &args, context) {
        return Ok(set);
    }

    // Resolve as commit hash.
    rev("lookup", repo, &args, context)
}

/// Resolve a function name.
fn get_function<'a>(
    name: &str,
    context: &'a Context,
) -> Result<&'a dyn Fn(&str, &Repo, &[Expr], &Context) -> Result<Set>> {
    if let Some(func) = context.fns.get(name) {
        return Ok(func);
    }
    match name {
        "parents" => Ok(&parents),
        "children" => Ok(&children),
        "ancestors" => Ok(&ancestors),
        "descendants" => Ok(&descendants),
        "heads" => Ok(&heads),
        "roots" => Ok(&roots),
        "range" => Ok(&range),
        "only" => Ok(&only),
        "ancestor" => Ok(&gca),
        "gca" => Ok(&gca),
        "intersection" => Ok(&intersection),
        "union" => Ok(&union),
        "difference" => Ok(&difference),
        "negate" => Ok(&negate),
        "first" => Ok(&first),
        "last" => Ok(&last),
        "head" => Ok(&head),
        "all" => Ok(&all),
        "publichead" => Ok(&publichead),
        "drafthead" => Ok(&drafthead),
        "public" => Ok(&public),
        "draft" => Ok(&draft),
        "author" => Ok(&author),
        "date" => Ok(&date),
        "committer" => Ok(&committer),
        "committerdate" => Ok(&committer_date),
        "desc" => Ok(&desc),
        "predecessors" => Ok(&predecessors),
        "successors" => Ok(&successors),
        "obsolete" => Ok(&obsolete),
        "rev" => Ok(&rev),
        "commit" => Ok(&rev),
        "ref" => Ok(&r#ref),
        _ => Err(Error::UnresolvedName(name.to_string())),
    }
}

/// Ensure the number of arguments.
fn ensure_arg_count(func_name: &str, args: &[Expr], n: usize, context: &Context) -> Result<()> {
    let _ = context;
    if args.len() != n {
        Err(Error::MismatchedArguments(
            func_name.to_string(),
            n,
            args.len(),
        ))
    } else {
        Ok(())
    }
}

/// Expr -> Set
fn resolve_set(repo: &Repo, expr: &Expr, context: &Context) -> Result<Set> {
    eval(repo, expr, context)
}

/// Expr -> String
fn resolve_string(expr: &Expr) -> Result<String> {
    match expr {
        Expr::Name(name) => Ok(name.clone()),
        _ => Err(Error::UnresolvedName(format!("{:?}", expr))),
    }
}

/// Resolve args to a single set.
fn resolve_single_set(
    func_name: &str,
    repo: &Repo,
    args: &[Expr],
    context: &Context,
) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    eval(repo, &args[0], context)
}

/// Resolve 2 args to 2 sets.
fn resolve_double_sets(
    func_name: &str,
    repo: &Repo,
    args: &[Expr],
    context: &Context,
) -> Result<(Set, Set)> {
    ensure_arg_count(func_name, args, 2, context)?;
    Ok((
        eval(repo, &args[0], context)?,
        eval(repo, &args[1], context)?,
    ))
}

fn parents(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    Ok(repo.dag().parents(set)?)
}

fn children(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    let dag = repo.dag();
    let roots = set.clone();
    let visible = dag.ancestors(dag.git_heads())?;
    Ok(dag.children(roots)? & visible)
}

fn ancestors(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    Ok(repo.dag().ancestors(set)?)
}

fn descendants(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    let dag = repo.dag();
    let roots = set.clone();
    Ok(dag.range(roots, dag.git_heads())? | set)
}

fn heads(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    Ok(repo.dag().heads(set)?)
}

fn roots(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    Ok(repo.dag().roots(set)?)
}

fn range(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let (roots, heads) = resolve_double_sets(func_name, repo, args, context)?;
    Ok(repo.dag().range(roots, heads)?)
}

fn only(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let (reachable, unreachable) = resolve_double_sets(func_name, repo, args, context)?;
    Ok(repo.dag().only(reachable, unreachable)?)
}

fn gca(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let _ = func_name;
    let mut set = repo.to_set(std::iter::empty())?;
    for arg in args {
        let subset = resolve_set(repo, arg, context)?;
        set = set | subset;
    }
    Ok(repo.dag().gca_all(set)?)
}

fn intersection(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let (a, b) = resolve_double_sets(func_name, repo, args, context)?;
    Ok(a & b)
}

fn union(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let (a, b) = resolve_double_sets(func_name, repo, args, context)?;
    Ok(a | b)
}

fn difference(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let (a, b) = resolve_double_sets(func_name, repo, args, context)?;
    Ok(a - b)
}

fn negate(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    let dag = repo.dag();
    Ok(dag.all()? - set)
}

fn first(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let _ = func_name;
    for arg in args {
        let subset = resolve_set(repo, arg, context)?;
        if let Some(v) = subset.first()? {
            return repo.to_set(std::iter::once(v));
        }
    }
    repo.to_set(std::iter::empty())
}

fn last(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    if let Some(v) = set.last()? {
        return repo.to_set(std::iter::once(v));
    }
    repo.to_set(std::iter::empty())
}

fn head(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    Ok(repo.dag().git_heads())
}

fn all(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    repo.cached_set("all", |repo| {
        let heads = head("head", repo, &[], context)?;
        Ok(repo.dag().ancestors(heads)?)
    })
}

fn publichead(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    repo.cached_set("publichead", |repo| {
        r#ref(
            "refglob",
            repo,
            &[Expr::Name("remotes/**".to_string())],
            context,
        )
    })
}

fn drafthead(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    repo.cached_set("drafthead", |repo| {
        Ok(head("head", repo, &[], context)? - publichead("publichead", repo, args, context)?)
    })
}

fn public(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    repo.cached_set("public", |repo| {
        let dag = repo.dag();
        Ok(dag.ancestors(publichead("publichead", repo, &[], context)?)?)
    })
}

fn draft(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    repo.cached_set("draft", |repo| {
        let dag = repo.dag();
        Ok(dag.ancestors(head("drafthead", repo, &[], context)?)?
            - public("public", repo, &[], context)?)
    })
}

fn author(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let name = resolve_string(&args[0])?;
    filter_set(repo, move |commit| {
        let author = commit.author();
        author.name().unwrap_or("").contains(&name) || author.email().unwrap_or("").contains(&name)
    })
}

fn date(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let date_str = resolve_string(&args[0])?;
    let date_range = match HgTime::parse_range(&date_str) {
        Some(range) => range.start.unixtime..=range.end.unixtime,
        None => return Err(crate::Error::ParseError(format!("invalid date: {}", date_str))),
    };
    filter_set(repo, move |commit| {
        let author = commit.author();
        let epoch = author.when().seconds();
        date_range.contains(&epoch)
    })
}

fn committer_date(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let date_str = resolve_string(&args[0])?;
    let date_range = match HgTime::parse_range(&date_str) {
        Some(range) => range.start.unixtime..=range.end.unixtime,
        None => return Err(crate::Error::ParseError(format!("invalid date: {}", date_str))),
    };
    filter_set(repo, move |commit| {
        let committer = commit.committer();
        let epoch = committer.when().seconds();
        date_range.contains(&epoch)
    })
}

fn committer(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let name = resolve_string(&args[0])?;
    filter_set(repo, move |commit| {
        let author = commit.committer();
        author.name().unwrap_or("").contains(&name) || author.email().unwrap_or("").contains(&name)
    })
}

fn desc(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let text = resolve_string(&args[0])?;
    filter_set(repo, move |commit| {
        commit.summary().unwrap_or("").contains(&text)
    })
}

fn predecessors(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    let dag = repo.dag();
    let mutdag = repo.mutation_dag()?;
    let set = set.clone() | (mutdag.ancestors(set & mutdag.all()?)? & dag.all()?);
    Ok(dag.sort(&set)?.flatten()?)
}

fn successors(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let set = resolve_single_set(func_name, repo, args, context)?;
    let dag = repo.dag();
    let mutdag = repo.mutation_dag()?;
    let set = set.clone() | (mutdag.descendants(set & mutdag.all()?)? & dag.all()?);
    Ok(dag.sort(&set)?.flatten()?)
}

fn obsolete(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 0, context)?;
    let mutdag = repo.mutation_dag()?;
    let draft = draft("draft", repo, &[], context)?;
    let obsoleted = mutdag.parents(draft.clone() & mutdag.all()?)?;
    let set = obsoleted & draft;
    Ok(repo.dag().sort(&set)?.flatten()?)
}

fn rev(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    ensure_arg_count(func_name, args, 1, context)?;
    let name = resolve_string(&args[0])?;
    match name.as_ref() {
        "." | "@" | "HEAD" => {
            let id = repo.git_repo().head()?.peel_to_commit()?.id();
            let v = Vertex::copy_from(id.as_bytes());
            repo.to_set(std::iter::once(v))
        }
        _ => {
            if let Some(bin_hex) = normalize_hex(&name) {
                let matched = repo.dag().vertexes_by_hex_prefix(&bin_hex, 3)?;
                match matched.len() {
                    0 => Err(Error::UnresolvedName(name)),
                    1 => repo.to_set(matched),
                    _ => Err(Error::AmbiguousPrefix(matched)),
                }
            } else {
                Err(Error::UnresolvedName(name))
            }
        }
    }
}

fn r#ref(func_name: &str, repo: &Repo, args: &[Expr], context: &Context) -> Result<Set> {
    let refs = repo.dag().git_references();
    // No arguments: all references.
    if args.len() == 0 {
        return repo.to_set(refs.values().cloned());
    }
    ensure_arg_count(func_name, args, 1, context)?;
    let name = resolve_string(&args[0])?;
    // Try precise lookup.
    if func_name != "refglob" {
        let candidates = [
            format!("refs/{}", name),
            format!("refs/heads/{}", name),
            format!("refs/tags/{}", name),
            format!("refs/remotes/{}", name),
        ];
        for name in candidates.iter() {
            if let Some(v) = refs.get(name) {
                return repo.to_set(std::iter::once(v.clone()));
            }
        }
    }
    // Try glob pattern lookup.
    if func_name != "lookup" && name.contains('*') {
        if let Ok(glob) = Glob::new(&format!("refs/{}", name)) {
            let matcher = glob.compile_matcher();
            let iter = refs
                .iter()
                .filter(|(k, _)| matcher.is_match(k))
                .map(|(_, v)| v.clone());
            return repo.to_set(iter);
        }
    }
    Err(Error::UnresolvedName(name))
}

fn normalize_hex(s: &str) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' => result.push(b),
            b'a'..=b'f' => result.push(b),
            b'A'..=b'F' => result.push(b - b'A' + b'a'),
            _ => return None,
        }
    }
    Some(result)
}

fn filter_set(
    repo: &Repo,
    func: impl Fn(&git2::Commit) -> bool + Send + Sync + 'static,
) -> Result<Set> {
    #[derive(Clone)]
    struct State {
        git_repo: Arc<Mutex<git2::Repository>>,
        func: Arc<dyn Fn(&git2::Commit) -> bool + Send + Sync + 'static>,
    }

    impl State {
        fn contains(&self, name: &Vertex) -> bool {
            if let Ok(oid) = git2::Oid::from_bytes(name.as_ref()) {
                if let Ok(commit) = self.git_repo.lock().unwrap().find_commit(oid) {
                    return self.func.deref()(&commit);
                }
            }
            false
        }
    }

    let state = State {
        git_repo: Arc::new(Mutex::new(git2::Repository::open(repo.git_repo().path())?)),
        func: Arc::new(func),
    };

    let evaluate = {
        let all = all("all", repo, &[], &Default::default())?;
        let state = state.clone();
        move || -> dag::Result<Set> {
            let iter = all
                .iter()?
                .filter(|name| match name {
                    Ok(name) => state.contains(name),
                    Err(_) => false,
                })
                .map(|name| name.unwrap());
            Ok(Set::from_static_names(iter.into_iter()))
        }
    };

    Ok(Set::from_evaluate_contains(evaluate, move |_, name| {
        Ok(state.contains(name))
    }))
}
