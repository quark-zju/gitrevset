use crate::ext::OidExt;
use crate::testrepo::TestRepo;

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
    assert_eq!(repo.query(r#"committerdate("before 2 0")"#), ["C", "B", "A"]);
    assert_eq!(repo.query(r#"committerdate("since 6 0")"#), ["I", "E", "D"]);

    // public(), draft()
    repo.add_ref("refs/heads/master", repo.query_single_oid("E"));
    repo.add_ref("refs/remotes/origin/master", repo.query_single_oid("D"));
    repo.add_ref("refs/remotes/origin/stable", repo.query_single_oid("B"));

    assert_eq!(repo.query("origin/master"), ["D"]);
    assert_eq!(repo.query("draft()"), ["E", "I", "H"]);
    assert_eq!(repo.query("public()"), ["D", "G", "F", "C", "B", "A"]);
    assert_eq!(repo.query("drafthead()"), ["E", "I"]);
    assert_eq!(repo.query("publichead()"), ["D", "B"]);

    // rev(), ref(), "."
    for name in repo.query("all()") {
        let rev_code = format!("rev({})", repo.query_single_oid(&name).to_vertex().to_hex());
        assert_eq!(repo.query(&rev_code), [name.clone()]);
    }
    assert_eq!(repo.query("ref(origin/master)"), ["D"]);
    assert_eq!(repo.query(r#"ref("remotes/origin/*")"#), ["D", "B"]);
    assert_eq!(repo.query("."), ["E"]);

    // predecessors(), successors()
    repo.amend("refs/heads/H");
    assert_eq!(repo.query("H"), ["H_new"]);
    assert_eq!(repo.query("H_old"), ["H"]);
    assert_eq!(repo.query("predecessors(H)"), ["H_new", "H"]);
    assert_eq!(repo.query("successors(H_old)"), ["H_new", "H"]);
    assert_eq!(repo.query("obsolete()"), ["H"]);
}
