{
  description = "Nix package for centrifuge-chain";

  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-20.09;

  outputs = inputs:
    let
      name = "centrifuge-chain";
      version = "2.0.0";
      pkgs = inputs.nixpkgs.legacyPackages.x86_64-linux;
    in
    {
      packages.x86_64-linux.centrifuge-chain =
        pkgs.rustPlatform.buildRustPackage {
          pname = name;
          version = version;

          src = inputs.self;

          cargoSha256 = "sha256-52CN7N9FQiJSODloo0VZGPNw4P5XsaWfaQxEf6Nm2gI=";

          nativeBuildInputs = with pkgs; [ clang pkg-config ];
          buildInputs = with pkgs; [ openssl ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang}/lib";
          PROTOC = "${pkgs.protobuf}/bin/protoc";
          BUILD_DUMMY_WASM_BINARY = 1;

          doCheck = false;
        };

      defaultPackage.x86_64-linux = inputs.self.packages.x86_64-linux.centrifuge-chain;

      packages.x86_64-linux.dockerContainer =
        pkgs.dockerTools.buildImage {
          name = "centrifugeio/${name}";
          tag = "latest";

          contents = inputs.self.defaultPackage.x86_64-linux;

          config = {
            Env = [
              "PATH=/bin:$PATH"
            ];
            ExposedPorts = {
              "30333/tcp" = { };
              "9933/tcp" = { };
              "9944/tcp" = { };
            };
            Volumes = {
              "/data" = { };
            };
            Entrypoint = [ "centrifuge-chain" ];
          };
        };

    };
}
