use crate::ext::OidExt;
use crate::testrepo::TestRepo;
use gitdag::dag::Set;

#[test]
fn test_revset_functions() {
    let mut repo = TestRepo::new();
    repo.drawdag(
        r#"
    A---B---C---D---E
         \     /
          F---G---H---I
    "#,
    );

    // Basic set operations.
    assert_eq!(
        repo.query("all()"),
        ["I", "H", "E", "D", "G", "F", "C", "B", "A"]
    );
    assert_eq!(repo.query("heads(all())"), ["I", "E"]);
    assert_eq!(repo.query("heads(A:C + G:H)"), ["H", "C"]);
    assert_eq!(repo.query("roots(all())"), ["A"]);
    assert_eq!(repo.query("roots(A:C + G:H)"), ["G", "A"]);
    assert_eq!(repo.query("B:D"), ["D", "G", "F", "C", "B"]);
    assert_eq!(repo.query("A:E - G - (C^ + C)^"), ["E", "D", "F", "C"]);
    assert_eq!(repo.query("!!!(B:D)"), ["I", "H", "E", "A"]);
    assert_eq!(repo.query("::B + G::"), ["I", "H", "E", "D", "G", "B", "A"]);
    assert_eq!(repo.query("H % E"), ["H"]);
    assert_eq!(repo.query("H % C"), ["H", "G", "F"]);
    assert_eq!(repo.query("gca(E+H)"), ["G"]);
    assert_eq!(repo.query("gca(E,H)"), ["G"]);
    assert_eq!(repo.query("first(A:D)"), ["D"]);
    assert_eq!(repo.query("first(B-B,C,D)"), ["C"]);
    assert_eq!(repo.query("first(B-B,C+D)"), ["D"]);
    assert_eq!(repo.query("last(A:D)"), ["A"]);
    assert_eq!(
        repo.query("children(G) | children(A:B)"),
        ["H", "D", "F", "C", "B"]
    );
    assert_eq!(repo.query("head()"), ["I", "E"]);
    assert_eq!(repo.query("desc(C)"), ["C"]);
    assert_eq!(repo.query("author(D)"), ["D"]);
    assert_eq!(repo.query("heads(author(test))"), ["I", "E"]);
    assert_eq!(repo.query("committer(E)"), ["E"]);
    assert_eq!(repo.query("heads(committer(test))"), ["I", "E"]);

    // date(), committerdate()
    assert_eq!(repo.query(r#"date("0 0")"#), ["B", "A"]);
    assert_eq!(repo.query(r#"date("0 0 to 1 0")"#), ["C", "B", "A"]);
    assert_eq!(
        repo.query(r#"committerdate("before 2 0")"#),
        ["C", "B", "A"]
    );
    assert_eq!(repo.query(r#"committerdate("since 6 0")"#), ["I", "E", "D"]);

    // public(), draft()
    repo.add_ref("refs/heads/master", repo.query_single_oid("E"));
    repo.add_ref("refs/remotes/origin/master", repo.query_single_oid("D"));
    repo.add_ref("refs/remotes/origin/stable", repo.query_single_oid("B"));
    repo.add_ref("refs/tags/v1", repo.query_single_oid("A"));
    repo.add_ref("refs/tags/v2", repo.query_single_oid("B"));

    assert_eq!(repo.query("origin/master"), ["D"]);
    assert_eq!(repo.query("draft()"), ["E", "I", "H"]);
    assert_eq!(repo.query("public()"), ["D", "G", "F", "C", "B", "A"]);
    assert_eq!(repo.query("drafthead()"), ["E", "I"]);
    assert_eq!(repo.query("publichead()"), ["D", "B"]);

    // id(), ref(), tag(), "."
    for name in repo.query("all()") {
        let rev_code = format!("id({})", repo.query_single_oid(&name).to_vertex().to_hex());
        assert_eq!(repo.query(&rev_code), [name.clone()]);
    }
    assert_eq!(
        repo.query("ref()"),
        ["E", "I", "H", "D", "G", "F", "C", "B", "A"]
    );
    assert_eq!(repo.query("ref(origin/master)"), ["D"]);
    assert_eq!(repo.query(r#"ref("remotes/origin/*")"#), ["D", "B"]);
    assert_eq!(repo.query("."), ["E"]);
    assert_eq!(repo.query("tag()"), ["B", "A"]);
    assert_eq!(repo.query("tag(v2)"), ["B"]);
    assert_eq!(repo.query(r#"tag("v*")"#), ["B", "A"]);

    // empty(), present()
    assert!(repo.query("empty()").is_empty());
    assert!(repo.query("present(foobar)").is_empty());
    assert_eq!(repo.query("present(master)"), ["E"]);

    // predecessors(), successors()
    repo.amend("refs/heads/H");
    assert_eq!(repo.query("H"), ["H_new"]);
    assert_eq!(repo.query("H_old"), ["H"]);
    assert_eq!(repo.query("predecessors(H)"), ["H_new", "H"]);
    assert_eq!(repo.query("successors(H_old)"), ["H_new", "H"]);
    assert_eq!(repo.query("obsolete()"), ["H"]);

    // apply
    assert_eq!(repo.query("apply($1, .)"), ["E"]);
    assert_eq!(repo.query("apply($1 + $2^, ., B)"), ["E", "A"]);
    assert_eq!(repo.query("apply(apply($1, C) + $1, A)"), ["C", "A"]);
}

#[test]
fn test_ast_macro() {
    use crate::ast;
    let f = |e| format!("{:?}", e);
    assert_eq!(f(ast!("foo")), "foo");
    assert_eq!(f(ast!(parents("foo"))), "parents(foo)");
    assert_eq!(f(ast!(draft())), "draft()");
    assert_eq!(
        f(ast!(union(draft(), public()))),
        "union(draft(), public())"
    );

    let name = "foo";
    assert_eq!(
        f(ast!(union(desc({ name }), author({ "bar" })))),
        "union(desc(foo), author(bar))"
    );

    let set = Set::from_static_names(vec!["A".into(), "B".into()]);
    assert_eq!(f(ast!(parents({ set }))), "parents(<static [A, B]>)")
}

#[test]
fn test_ast_repo() -> crate::Result<()> {
    use crate::ast;
    let mut repo = TestRepo::new();
    repo.drawdag("A-B-C-D");
    repo.add_ref("refs/heads/master", repo.query_single_oid("D"));
    repo.add_ref("refs/remotes/origin/master", repo.query_single_oid("B"));

    let master = "origin/master";
    let stack = repo.revs(ast!(only(".", ref({ master })))).unwrap();
    assert_eq!(repo.desc_set(&stack), ["D", "C"]);
    let head = repo.revs(ast!(heads({ stack }))).unwrap();
    assert_eq!(repo.desc_set(&head), ["D"]);
    Ok(())
}
