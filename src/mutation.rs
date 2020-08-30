use crate::ext::Merge;
use crate::ext::OidExt;
use crate::repo::Repo;
use crate::Result;
use dag::namedag::MemNameDag;
use dag::ops::DagAddHeads;
use dag::Vertex;
use gitdag::dag;
use gitdag::git2;
use std::collections::HashMap;
use std::collections::HashSet;

pub(crate) fn infer_mutation_from_reflog(repo: &Repo) -> Result<MemNameDag> {
    let refs = repo.dag().git_references();
    let mut replaces: HashMap<Vertex, Vertex> = Default::default();
    for name in refs.keys() {
        if !name.starts_with("refs/remotes/") && name.starts_with("refs/heads/") {
            replaces.merge(analyse_reflog_name(repo, name).unwrap_or_default());
        }
    }

    let parent_func = |v: Vertex| -> dag::Result<Vec<Vertex>> {
        match replaces.get(&v) {
            None => Ok(Vec::new()),
            Some(old) => Ok(vec![old.clone()]),
        }
    };
    let parent_func = dag::utils::break_parent_func_cycle(parent_func);
    let mut heads: Vec<Vertex> = replaces
        .keys()
        .collect::<HashSet<_>>()
        .difference(&replaces.values().collect::<HashSet<_>>())
        .cloned()
        .cloned()
        .collect();
    heads.sort_unstable();

    let mut dag = MemNameDag::new();
    dag.add_heads(parent_func, &heads)?;
    Ok(dag)
}

fn analyse_reflog_name(repo: &Repo, name: &str) -> Result<HashMap<Vertex, Vertex>> {
    // Check reflog for the given reference name.
    let reflog = repo.git_repo().reflog(name)?;
    let mut replaces: HashMap<Vertex, Vertex> = Default::default();
    for entry in reflog.iter() {
        let message: &str = match entry.message() {
            Some(m) => m,
            None => continue,
        };
        if message.starts_with("commit (amend):") || message.starts_with("rebase -i (finish):") {
            replaces.merge(
                analyse_head_rewrite(repo.git_repo(), entry.id_old(), entry.id_new())
                    .unwrap_or_default(),
            );
        }
    }
    Ok(replaces)
}

fn analyse_head_rewrite(
    git_repo: &git2::Repository,
    mut old: git2::Oid,
    mut new: git2::Oid,
) -> Result<HashMap<Vertex, Vertex>> {
    const MAX_DEPTH: usize = 50;

    // Find the old and new stack. Not using "dag" APIs as "dag" could be
    // incomplete (i.e. not indexing the reflog heads).
    //
    // Because "dag" API is not used, this is an approximate. For example,
    // `old_stack` might be ancestors of `new_stack`, and common ancestors
    // might be inserted to either `old_stack` or `new_stack`.
    let mut old_stack = Vec::new();
    let mut new_stack = Vec::new();
    let mut seen = HashSet::new();
    for _ in 0..MAX_DEPTH {
        if old == new {
            break;
        }
        if seen.insert(old) {
            old_stack.push(old);
            if let Some(next_old) = git_repo.find_commit(old)?.parent_ids().next() {
                old = next_old;
            }
        }
        if seen.insert(new) {
            new_stack.push(new);
            if let Some(next_new) = git_repo.find_commit(old)?.parent_ids().next() {
                new = next_new;
            }
        }
    }

    // If tree id is not changed, then it's considered as a "commit message"
    // rewrite. To detect that, keep {tree: [commit]} map.
    // If commit message and author date is not changed, then it's consider
    // as a "content" rewrite. To detect that, keep {(msg, date): [commit]}
    // map.

    #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
    enum Side {
        Old,
        New,
    }
    let mut tree_map = HashMap::<git2::Oid, Vec<(Side, git2::Oid)>>::new();
    let mut msg_map = HashMap::<(i64, Vec<u8>), Vec<(Side, git2::Oid)>>::new();

    for (side, stack) in vec![(Side::Old, old_stack), (Side::New, new_stack)] {
        for oid in stack.into_iter().take(MAX_DEPTH) {
            let commit = match git_repo.find_commit(oid) {
                Err(_) => continue,
                Ok(commit) => commit,
            };
            let tree_id = commit.tree_id();
            tree_map.entry(tree_id).or_default().push((side, oid));
            let msg = commit.message_bytes().to_vec();
            let time = commit.author().when().seconds();
            msg_map.entry((time, msg)).or_default().push((side, oid));
        }
    }

    let result = tree_map
        .values()
        .chain(msg_map.values())
        .filter_map(|pair| {
            if let [(Side::Old, old), (Side::New, new)] = &pair[..] {
                if old == new {
                    None
                } else {
                    Some((new.to_vertex(), old.to_vertex()))
                }
            } else {
                None
            }
        })
        .collect();

    Ok(result)
}
