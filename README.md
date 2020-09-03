# gitrevset

A domain-specific-language to select commits in a git repo. Similar to
[Mercurial's revset](https://www.mercurial-scm.org/repo/hg/help/revsets).

See the crate documentation for supported functions and operators. More functions might be added over time.

`gitrevset` provides the Rust library interface. There is also a simple command-line utility `git-revs`. It takes revset expressions as arguments, and outputs commit hashes.

## Examples

### Revset Expressions

The current commit (HEAD) and its parent:

    . + .^

Merge base (common ancestor) of HEAD and origin/master:

    gca(., origin/master)

The bottom of the current local (draft) branch:

    roots(draft() & ::.)

Commits by "alice" or "bob" in the "dev" but not "master" branch:

    (dev % master) & (author(alice) | author(bob))

Heads of local (draft) commits, excluding commits with "fixup" in message and their descendants:

    heads(draft() - (draft() & desc(fixup))::)

### Using `gitrevset` Library

```rust
use gitrevset::{Repo, SetExt};

let repo = Repo::open_from_env()?;
let set = repo.revs("(draft() & ::.)^ + .")?;
for oid in set.to_oids()? {
    dbg!(oid?)
}
```

### Using `git-revs` CLI

```bash
git revs "(draft() & ::.)^ + ."
```