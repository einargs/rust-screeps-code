{ lib, buildNpmPackage, fetchFromGitHub, nodejs_10 }:

buildNpmPackage rec {
  pname = "screeps";
  version = "4.2.16";

  src = fetchFromGitHub {
    owner = "screeps";
    repo = "screeps";
    # The tagged versions are out of date
    rev = "029417f1490cac25e639b8f1bd35a49a50c37791";
    hash = "sha256-tfVNkjhEX6mQWNK+4Ag+UUj5efoALIe+K/CVigYOMvc=";
  };

  # without patch
  npmDepsHash = "sha256-a0vhDYsGDpRsCE2EbRiD35Q5KUJEUb8Sto4P/vopBA8=";
  # with patch
  # npmDepsHash = "sha256-rKa5/I3QXnxZOK0gJBqIcZxa3B5Q0HpTiU9e2+b6en4=";
  # patches = [ ./package-screeps.patch ];

  npmFlags = [ "--legacy-peer-deps" ];
  makeCacheWritable = true;
  dontNpmBuild = true;
  nodejs = nodejs_10;

  # The prepack script runs the build script, which we'd rather do in the build phase.
  npmPackFlags = [ "--ignore-scripts" ];

  NODE_OPTIONS = "--openssl-legacy-provider";

  meta = with lib; {
    description = "A modern web UI for various torrent clients with a Node.js backend and React frontend";
    homepage = "https://github.com/screeps/screeps";
  };
}
