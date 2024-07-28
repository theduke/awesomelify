{
  description = "awesomelify";

  inputs = {
    flakeutils = {
      url = "github:numtide/flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flakeutils }:
    flakeutils.lib.eachDefaultSystem (system:
      let
        NAME = "awesomelify";
        VERSION = "0.1";

        pkgs = import nixpkgs {
          inherit system;
        };

        lib = pkgs.lib;
        hasPrefix = lib.hasPrefix;
      in
      rec {
        packages.${NAME} = pkgs.rustPlatform.buildRustPackage rec {
          pname = NAME;
          version = VERSION;

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [
          ];

          buildInputs = [
          ];

          checkPhase = ''
            echo tests...
          '';

          meta = {
            description = "awesomelify server";
            homepage = "https://github.com/theduke/awesomelify";
            license = pkgs.lib.licenses.mit;
            maintainers = [];
          };
        };

        defaultPackage = packages.${NAME};

        # For `nix run`.
        apps.${NAME} = flakeutils.lib.mkApp {
          drv = packages.${NAME};
        };
        defaultApp = apps.${NAME};

        packages.dockerImage = pkgs.dockerTools.buildImage {
          name = "theduke/${NAME}";
          copyToRoot = [
            packages.${NAME}
          ];
          config = {
            Cmd = [ "/bin/awesomelify" "serve" ];
          };
        };

        devShell = pkgs.mkShell {
          env.RUST_LOG = "awesomelify=trace,reels_db=trace,info";

          packages = with pkgs; [
            just
            cargo-nextest
          ];
        };
      }
    );
}
