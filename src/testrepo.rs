use crate::git2;
use crate::Repo;
use crate::SetExt;
use git2::Oid;
use std::ops::Deref;
use gitdag::dag::Set;

/// Repo for testing purpose.
pub struct TestRepo {
    dir: tempfile::TempDir,
    repo: Repo,
}

impl TestRepo {
    /// Create a test repo.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let git_repo = git2::Repository::init(&dir.path()).unwrap();
        let repo = Repo::open_from_repo(Box::new(git_repo)).unwrap();
        Self { dir, repo }
    }

    /// Add commits using the ASCII graph.
    pub fn drawdag(&mut self, ascii: &str) {
        let repo = self.repo.git_repo();
        let mut epoch = 0;
        drawdag::drawdag(
            ascii,
            |name: String, parents: Vec<Box<[u8]>>| -> Box<[u8]> {
                let parents: Vec<_> = parents
                    .into_iter()
                    .map(|s| {
                        let oid = git2::Oid::from_bytes(&s).unwrap();
                        repo.find_commit(oid).unwrap()
                    })
                    .collect();
                let parent_refs: Vec<_> = parents.iter().collect();
                let time = git2::Time::new(epoch, 0);
                epoch += 1;
                let sig = git2::Signature::new(&name, "test@example.com", &time).unwrap();
                let mut tree_builder = repo.treebuilder(None).unwrap();
                let blob_oid = repo.blob(name.as_bytes()).unwrap();
                tree_builder.insert(&name, blob_oid, 0o100644).unwrap();
                let tree_id = tree_builder.write().unwrap();
                let tree = repo.find_tree(tree_id).unwrap();
                let commit_id = repo
                    .commit(None, &sig, &sig, &name, &tree, &parent_refs)
                    .unwrap();
                repo.reference(&format!("refs/heads/{}", &name), commit_id, true, "commit")
                    .unwrap();
                commit_id.as_bytes().to_vec().into_boxed_slice()
            },
        );
        self.reload();
    }

    /// Run revset query. Return commit messages.
    pub fn query(&self, code: &str) -> Vec<String> {
        self.desc_set(&self.revs(code).unwrap())
    }

    /// Set -> Commit messages.
    pub fn desc_set(&self, set: &Set) -> Vec<String> {
        let mut result = Vec::new();
        for oid in set.to_oids().unwrap() {
            let oid = oid.unwrap();
            let commit = self.git_repo().find_commit(oid).unwrap();
            let message = commit.message().unwrap();
            result.push(message.to_string());
        }
        result
    }

    /// Run revset query. Resolve to a single commit `Oid`.
    pub fn query_single_oid(&self, code: &str) -> Oid {
        self.revs(code)
            .unwrap()
            .to_oids()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
    }

    /// Add a reference.
    pub fn add_ref(&mut self, name: &str, oid: Oid) {
        let dir = self.repo.git_repo().path();
        let git_repo = git2::Repository::init(dir).unwrap();
        git_repo.reference(name, oid, true, "add_ref").unwrap();
        self.reload();
    }

    /// Make "commit (amend)" change to a reference.
    pub fn amend(&mut self, ref_name: &str) {
        let dir = self.repo.git_repo().path();
        let git_repo = git2::Repository::init(dir).unwrap();
        let oid = git_repo.refname_to_id(ref_name).unwrap();
        let commit = git_repo.find_commit(oid).unwrap();
        let msg = commit.message().unwrap();
        let new_msg = format!("{}_new", msg);
        let new_oid = commit
            .amend(None, None, None, None, Some(&new_msg), None)
            .unwrap();
        git_repo
            .reference(ref_name, new_oid, true, "commit (amend): amend")
            .unwrap();
        let old_ref_name = format!("{}_old", ref_name);
        git_repo
            .reference(&old_ref_name, oid, true, "before amend")
            .unwrap();
        self.reload();
    }

    /// Update environment variable so open_from_env will open this repo.
    pub fn set_env(&self) {
        std::env::set_var("GIT_DIR", self.repo.git_repo().path());
    }

    /// Reload the test repo. Pick up changes made via the git2 repo.
    pub fn reload(&mut self) {
        let dir = self.repo.git_repo().path();
        let git_repo = git2::Repository::init(dir).unwrap();
        let repo = Repo::open_from_repo(Box::new(git_repo)).unwrap();
        self.repo = repo;
    }
}

impl Deref for TestRepo {
    type Target = Repo;
    fn deref(&self) -> &Repo {
        &self.repo
    }
}
