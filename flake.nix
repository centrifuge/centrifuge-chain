{
  description = "Nix package for centrifuge-chain";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-20.09;
    gitignore-nix = {
      url = github:hercules-ci/gitignore.nix;
      flake = false;
    };
  };

  outputs = inputs:
    let
      name = "centrifuge-chain";
      version = "2.0.0";
      gitignore = (import inputs.gitignore-nix { inherit (inputs.nixpkgs.legacyPackages.x86_64-linux) lib; }).gitignoreSource;
    in
    {
      defaultPackage.x86_64-linux =
        with import inputs.nixpkgs { system = "x86_64-linux"; };

        rustPlatform.buildRustPackage {
          pname = name;
          version = version;

          src = gitignore ./.;

          cargoSha256 = "sha256-52CN7N9FQiJSODloo0VZGPNw4P5XsaWfaQxEf6Nm2gI=";

          nativeBuildInputs = [ clang pkg-config ];
          buildInputs = [ openssl ];

          LIBCLANG_PATH = "${llvmPackages.libclang}/lib";
          PROTOC = "${protobuf}/bin/protoc";
          BUILD_DUMMY_WASM_BINARY = 1;

          doCheck = false;
        };

      packages.x86_64-linux.dockerContainer =
        let
          pkgs = inputs.nixpkgs.legacyPackages.x86_64-linux;
        in
        pkgs.dockerTools.buildImage {
          name = "centrifugeio/${name}";
          tag = "latest";

          contents = inputs.self.defaultPackage.x86_64-linux;

          config = {
            Env = [
              "PATH=/bin/centrifuge-chain"
            ];
            ExposedPorts = {
              "30333/tcp" = { };
              "9933/tcp" = { };
              "9944/tcp" = { };
            };
            Volumes = {
              "/data" = { };
            };
            Entrypoint = [ "/bin/centrifuge-chain" ];
          };
        };

    };
}
