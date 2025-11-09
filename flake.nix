{
  description = ''
    Project development environment and dependency management flake.

    Provides:
      - Separate interactive and ad-hoc dev shells
      - Nixfmt for formatting Nix expressions
        ↪ Ref: https://github.com/NixOS/nixfmt?tab=readme-ov-file#nix-fmt-experimental
  '';

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-darwin" ] (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        nixfmt = nixpkgs.legacyPackages.${system}.nixfmt-tree;

        baseShell = pkgs.mkShell {
          buildInputs = [
            nixfmt
            (pkgs.python312.withPackages (
              ps: with ps; [
                mdformat
                mdformat-gfm
              ]
            ))
            pkgs.shellcheck
            pkgs.shfmt
          ];

          shellHook = ''
            shfmt() {
              command shfmt -i 2 -ci -s "$@"
            }
          '';
        };

        interactiveShell = pkgs.mkShell {
          buildInputs = baseShell.buildInputs;
          shellHook = baseShell.shellHook + ''
            red="\e[91m"
            green="\e[32m"
            blue="\e[34m"
            bold="\e[1m"
            reset="\e[0m"

            projectName="$(basename "$PWD")"

            echo -e "''${blue}Entering Brewfile linting environment...''${reset}\n"

            echo -e "''${bold}Project:''${reset} ''${green}''${projectName}''${reset}"

            echo -e "''${bold}Nix version:''${reset} ''${red}$(nix --version | cut -d' ' -f2-)''${reset}"
            echo -e "''${bold}Nix fmt version:''${reset} ''${red}$(nix fmt -- --version)''${reset}"

            echo -e "''${bold}Python version:''${reset} ''${red}$(python --version | awk '{print $2}')''${reset}"
            echo -e "''${bold}mdformat version:''${reset} ''${red}$(mdformat --version | cut -d' ' -f2-)''${reset}"

            echo -e "''${bold}Bash version:''${reset} ''${red}$(bash --version | head -n 1)''${reset}"
            echo -e "''${bold}shellcheck version:''${reset} ''${red}$(shellcheck --version | awk '/^version:/ {print $2}')''${reset}"
            echo -e "''${bold}shfmt version:''${reset} ''${red}$(shfmt --version)''${reset}"
          '';
        };
      in
      {
        devShells.default = interactiveShell;
        devShells.adhoc = baseShell;
        formatter = nixfmt;
      }
    );
}
