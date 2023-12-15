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
  let pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        nodejs_20
        nodePackages.pnpm
        rustup
      ];
      src = [
        ./flake.nix
        ./flake.lock
      ];
      shellHook = ''
      '';
    };
  });
}
