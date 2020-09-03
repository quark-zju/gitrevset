use gitrevset::Repo;
use gitrevset::Result;
use std::env;

fn try_main()-> Result<()> {
    let repo = Repo::open_from_env()?;
    for arg in env::args().skip(1) {
        let arg: &str = &arg;
        let set = repo.revs(arg)?;
        for v in set.iter()? {
            println!("{}", v?.to_hex());
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