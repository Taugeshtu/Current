{
  description = "Current - desktop attention context resolver";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            version = cargoToml.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = [
              pkgs.gtk3
            ];

            postInstall = ''
              mkdir -p $out/lib
              $CC -shared -fPIC -O2 -Wall -Wextra \
                -o $out/lib/libthunar-current.so \
                ./thunar/thunar-current.c \
                $(pkg-config --cflags --libs gtk+-3.0)
            '';
          };
        }
      );

      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            buildInputs = [
              pkgs.cargo
              pkgs.rustc
            ];
          };
        }
      );
    };
}
