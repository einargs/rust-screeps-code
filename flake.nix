{
  description = "Tool for doctors to summarize doctor-patient conversations";

  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  nixConfig = {
    bash-prompt = ''\[\033[1;32m\][\[\e]0;\u@\h: \w\a\]dev-shell:\w]\$\[\033[0m\] '';
  };

  outputs = { self, nixpkgs, flake-utils }: 
  flake-utils.lib.eachDefaultSystem (system:
  let pkgs = import nixpkgs {
    inherit system;
    config.permittedInsecurePackages = [ "python-2.7.18.7" "nodejs-10.24.0"];
  };
      buildNodejs = pkgs.callPackage "${nixpkgs}/pkgs/development/web/nodejs/nodejs.nix" {};
      nodejs_10 = buildNodejs {
        enableNpm = true;
        version = "10.24.0";
        sha256 = "1k1srdis23782hnd1ymgczs78x9gqhv77v0am7yb54gqcspp70hm";
      };
      screeps = pkgs.callPackage ./client.nix {inherit nodejs_10;};
  in {
    devShells.default = pkgs.mkShell {
      nativeBuildInputs = with pkgs; [
        pkg-config
      ];
      buildInputs = with pkgs; [
        openssl
        openssl.dev
        nodejs_20
        screeps
        python2
        rustup
      ];
      src = [
        ./flake.nix
        ./flake.lock
      ];
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl ];
      shellHook = ''
      '';
    };
  });
}
