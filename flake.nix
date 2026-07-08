{
  description = "canonic: versioned Jira canned-response corpus CLI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system:
          f {
            inherit system;
            pkgs = import nixpkgs { inherit system; };
          });
    in {
      packages = forAllSystems ({ pkgs, system }: rec {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "canonic";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = false;
        };

        # Free-tier Jira REST stand-in image (python fixture; no Marketplace apps).
        jira-fixture-image = pkgs.dockerTools.buildImage {
          name = "canonic-jira-fixture";
          tag = "latest";
          copyToRoot = pkgs.buildEnv {
            name = "canonic-jira-fixture-root";
            paths = [
              pkgs.python3
              (pkgs.runCommand "canonic-jira-fixture-app" { } ''
                mkdir -p $out/app
                cp ${./scripts/jira-fixture/server.py} $out/app/server.py
              '')
            ];
            pathsToLink = [ "/bin" "/app" ];
          };
          config = {
            Cmd = [ "${pkgs.python3}/bin/python3" "-u" "/app/server.py" ];
            ExposedPorts = { "8080/tcp" = { }; };
            Env = [ "CANONIC_JIRA_FIXTURE_PORT=8080" ];
          };
        };
      });

      checks = forAllSystems ({ pkgs, system }: {
        jira-fixture-image = self.packages.${system}.jira-fixture-image;
      });

      devShells = forAllSystems ({ pkgs, system }: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.pandoc
            pkgs.python3
            pkgs.curl
          ];
        };
      });
    };
}
