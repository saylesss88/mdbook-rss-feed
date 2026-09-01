{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";

        mkMdbookRssFeed =
          {
            withAtom ? false,
            withJsonFeed ? false,
          }:
          let
            manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
            buildFeatures =
              pkgs.lib.optional withAtom "atom"
              ++ pkgs.lib.optional withJsonFeed "json-feed";
          in
          pkgs.rustPlatform.buildRustPackage {
            pname = manifest.name;
            version = manifest.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            inherit buildFeatures;
            meta = with pkgs.lib; {
              description = manifest.description;
              homepage = manifest.repository;
              license = licenses.asl20;
              mainProgram = "mdbook-rss-feed";
            };
          };
      in
      {
        packages.default = mkMdbookRssFeed { };
        packages.full = mkMdbookRssFeed {
          withAtom = true;
          withJsonFeed = true;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.rustup.out
            pkgs.mdbook.out
            pkgs.gnugrep.out
          ];
        };
      }
    );
}
