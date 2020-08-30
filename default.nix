with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "gitrevset";
  # used by the git2 crate
  buildInputs = [ openssl pkg-config ];
}
