use gitrevset::Expr;
use gitrevset::Repo;
use gitrevset::Result;
use std::env;

fn try_main() -> Result<()> {
    let repo = Repo::open_from_env()?;
    let mut print_ast = false;
    for arg in env::args().skip(1) {
        let arg: &str = &arg;
        if arg == "--ast" {
            print_ast = true;
            continue;
        }
        if print_ast {
            let ast = Expr::parse(arg)?;
            println!("{:?}", ast);
        } else {
            let set = repo.anyrevs(arg)?;
            for v in set.iter()? {
                println!("{}", v?.to_hex());
            }
        }
    }
    Ok(())
}

fn main() {
    match try_main() {
        Ok(()) => (),
        Err(e) => eprintln!("{}", e),
    }
}
