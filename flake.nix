{
  description = "Fyle Linux file manager and native source editor";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/55bfb4d7437bd4e09ab7cba58a43bd68eb310044";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      packageFor = system:
        let
          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;
          graphicsLibraries = with pkgs; [
            libxkbcommon
            wayland
            libGL
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
          ];
        in pkgs.rustPlatform.buildRustPackage {
          pname = "fyle";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.git pkgs.makeWrapper pkgs.pkg-config ];
          buildInputs = graphicsLibraries;

          postInstall = ''
            install -Dm644 ${./fyle.desktop} $out/share/applications/fyle.desktop
            install -Dm644 ${./assets/fyle.svg} $out/share/icons/hicolor/scalable/apps/fyle.svg
            for executable in $out/bin/*; do
              wrapProgram "$executable" --prefix PATH : ${lib.makeBinPath [ pkgs.git pkgs.xdg-utils ]}
            done
          '';

          postFixup = ''
            for executable in $out/bin/*; do
              patchelf --add-rpath ${lib.makeLibraryPath graphicsLibraries} "$executable"
            done
          '';

          meta = {
            description = "A cyberpunk desktop file manager and native source editor";
            homepage = "https://github.com/seridian-dev/fyle";
            license = lib.licenses.mit;
            mainProgram = "fyle";
            platforms = systems;
          };
        };
    in {
      packages = eachSystem (system: {
        default = packageFor system;
        fyle = packageFor system;
      });
      apps = eachSystem (system: {
        default = {
          type = "app";
          program = "${packageFor system}/bin/fyle";
        };
      });
    };
}
