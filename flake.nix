{
  description = "Nix package for centrifuge-chain";

  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-20.09;

  outputs = { self, nixpkgs, ... }: {
        defaultPackage.x86_64-linux =
          with import nixpkgs { system = "x86_64-linux"; };
          rustPlatform.buildRustPackage {
            pname = "centrifuge-chain";
            version = "2.0.0";

            src = self;

            cargoSha256 = "sha256-zB7PO3woCV9r3VYWzsk/DmUIc8+LVf86rWhG9FEpD18=";

            BUILD_DUMMY_WASM_BINARY = 1;
          };
  };
}
