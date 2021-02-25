{
  description = "Nix package for centrifuge-chain";

  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-20.09;

  outputs = { self, nixpkgs, ... }: let
    name = "centrifuge-chain";
    version = "2.0.0";
  in
    {
      defaultPackage.x86_64-linux =
        with import nixpkgs { system = "x86_64-linux"; };

        rustPlatform.buildRustPackage {
          pname = name;
          version = version;

          src = self;

          cargoSha256 = "sha256-zB7PO3woCV9r3VYWzsk/DmUIc8+LVf86rWhG9FEpD18=";

          nativeBuildInputs = [ clang pkg-config ];
          buildInputs = [ openssl ];

          LIBCLANG_PATH = "${llvmPackages.libclang}/lib";
          PROTOC = "${protobuf}/bin/protoc";
          BUILD_DUMMY_WASM_BINARY = 1;

          doCheck = false;
        };

      packages.x86_64-linux.dockerContainer = let
        pkgs = import nixpkgs { system = "x86_64-linux"; };
      in
        pkgs.dockerTools.buildImage {
          name = "centrifugeio/${name}";
          tag = "latest";

          config = {
            ExposedPorts = [ 30333 9933 9944 ];
            Volumes = {
                "/data" = {};
            };
            Cmd = [ "${self.defaultPackage.x86_64-linux}/bin/centrifuge-chain" ];
          };
      };

    };
}
