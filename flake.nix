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
    config.permittedInsecurePackages = [ "python-2.7.18.7" ];
  };
  in {
    devShells.default = pkgs.mkShell {
      nativeBuildInputs = with pkgs; [
        pkg-config
      ];
      buildInputs = with pkgs; [
        openssl
        openssl.dev
        nodejs_20
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
