# treefmt configuration, evaluated by treefmt-nix.lib.evalModule in flake.nix.
#
# This is the single lint/format orchestrator for the repo (it replaced the
# hand-rolled scripts/lint.sh). Style rules preserved from the old script:
#   - shell: shfmt -i 2 -ci -s; shellcheck (root .shellcheckrc disables SC1090/SC1091)
#   - markdown: mdformat + GFM plugin, honoring .mdformat.toml (105-column wrap)
#   - nix: nixfmt (RFC 166 style — pkgs.nixfmt is nixfmt-classic in nixpkgs 25.05,
#     so the package is overridden explicitly)
#   - TOML: taplo, with dot_aerospace.toml excluded (user-preferred visual alignment)
#   - JSON/YAML: validation-only via jq/yq (custom formatters below)
#
# Validators are treefmt "formatters" that never write; a non-zero exit fails the
# run — and the flake's treefmt check derivation, so `nix flake check` gates them.
{ pkgs, ... }:
let
  # `jq empty` per file: exit non-zero on any JSON parse error.
  jqValidate = pkgs.writeShellApplication {
    name = "jq-validate";
    runtimeInputs = [ pkgs.jq ];
    text = ''
      status=0
      for file do
        jq empty <"$file" || {
          echo "jq-validate: invalid JSON: $file" >&2
          status=1
        }
      done
      exit "$status"
    '';
  };

  # `yq eval` per file: exit non-zero on any YAML parse error.
  yqValidate = pkgs.writeShellApplication {
    name = "yq-validate";
    runtimeInputs = [ pkgs.yq-go ];
    text = ''
      status=0
      for file do
        yq eval '.' "$file" >/dev/null || {
          echo "yq-validate: invalid YAML: $file" >&2
          status=1
        }
      done
      exit "$status"
    '';
  };

  # Chezmoi shell templates contain Go template syntax that shellcheck can't
  # parse directly: render first, then shellcheck the result. CI=1 keeps the
  # templates on their non-interactive branch; --source "$PWD" so
  # includeTemplate resolves against this checkout's .chezmoitemplates
  # (treefmt runs formatters from the tree root). The per-file body lives in
  # scripts/lib-shellcheck-rendered-template.sh (sourced verbatim below) so
  # test/unit/rendered-template-shellcheck-wrapper.sh can drive its blank-render skip
  # semantic with a stubbed chezmoi and shellcheck.
  shellcheckRenderedTemplate = pkgs.writeShellApplication {
    name = "shellcheck-rendered-template";
    runtimeInputs = [
      pkgs.chezmoi
      pkgs.shellcheck
    ];
    text = ''
      # chezmoi needs a writable HOME; the Nix check sandbox has none.
      HOME="$(mktemp -d)"
      export HOME
      ${builtins.readFile ./scripts/lib-shellcheck-rendered-template.sh}
      status=0
      for file do
        render_and_shellcheck_one "$file" || status=1
      done
      exit "$status"
    '';
  };

  # osquery's config and packs are JSON-bodied .conf templates assembled by
  # run_onchange_before_50-setup-osquery.sh.tmpl via includeTemplate. The plain
  # *.json validator never sees them, and a broken config silently stops the
  # daemon from loading. Render each (osquery.conf carries {{ .chezmoi.homeDir }}
  # directives) and jq-validate the result.
  osqueryConfigRender = pkgs.writeShellApplication {
    name = "osquery-config-render";
    runtimeInputs = [
      pkgs.chezmoi
      pkgs.jq
    ];
    text = ''
      HOME="$(mktemp -d)"
      export HOME
      status=0
      for file do
        # Source path -> includeTemplate name (drop the .chezmoitemplates/ prefix).
        tmpl="''${file#./}"
        tmpl="''${tmpl#.chezmoitemplates/}"
        CI=1 chezmoi --source "$PWD" execute-template --no-tty \
          "{{ includeTemplate \"''${tmpl}\" . }}" | jq empty || {
          echo "osquery-config-render: rendered config is not valid JSON: $file" >&2
          status=1
        }
      done
      exit "$status"
    '';
  };

  # Programmatic discovery of the safely renderable shell templates handed to the
  # shellcheck-rendered-template formatter below. The 2026-07-10 audit found the
  # old hand-list covered 6 of ~20 shell templates, hiding four render failures.
  # The set is every `.chezmoiscripts/*.sh.tmpl` plus the shell `dot_*.tmpl` at
  # the repo root, MINUS any template that cannot render headless: one that (or
  # whose included partials, transitively) invokes keepassxc, or that has an
  # includeTemplate name which cannot be resolved statically. Both predicates
  # come from the importable, builtins-only classifier in
  # scripts/render-coverage-classifier.nix. That SAME file is what
  # test/integration/rendered-template-coverage.sh drives through a fixture matrix via
  # `nix eval`, so weakening a predicate there fails the fixtures rather than
  # silently passing while this list stays unchanged.
  classifier = import ./scripts/render-coverage-classifier.nix;
  includeBase = ./.chezmoitemplates;

  chezmoiscriptShellTemplates =
    let
      dir = ./.chezmoiscripts;
      entries = builtins.readDir dir;
      names = builtins.filter (
        name:
        entries.${name} == "regular"
        && builtins.match ".*\\.sh\\.tmpl" name != null
        && !(classifier.rendersUnsafe includeBase (dir + "/${name}"))
      ) (builtins.attrNames entries);
    in
    map (name: ".chezmoiscripts/${name}") names;

  rootShellDotTemplates =
    let
      dir = ./.;
      entries = builtins.readDir dir;
    in
    builtins.filter (
      name:
      entries.${name} == "regular"
      && builtins.match "dot_.*\\.tmpl" name != null
      && classifier.isShellTemplate (dir + "/${name}")
      && !(classifier.rendersUnsafe includeBase (dir + "/${name}"))
    ) (builtins.attrNames entries);

  renderedShellTemplates = builtins.sort (a: b: a < b) (
    chezmoiscriptShellTemplates ++ rootShellDotTemplates
  );
in
{
  projectRootFile = "flake.nix";

  # The ONE global prune set (replaces lint.sh's 6x-duplicated `find -prune`
  # list). .git is never walked; the rest are belt-and-braces for the
  # filesystem walker (the git walker and the flake-source copy already skip
  # gitignored paths).
  settings.excludes = [
    ".direnv/**"
    # Band-aid: droppable once the managed post-commit dispatcher is applied
    # live — .githooks/no-graphify then stops graphify writing here at all.
    "graphify-out/**"
    ".superpowers/**"
    ".worktrees/**"
    "**/vendor/**"
    "**/.vendor/**"
    "**/node_modules/**"
  ];

  # Shell — the old lint.sh shell set: *.sh, *.bash, dot_bash*, dot_profile
  # (never *.tmpl; rendered templates are handled below).
  programs.shellcheck.enable = true;
  programs.shellcheck.includes = [
    "*.sh"
    "*.bash"
    "dot_bash*"
    "dot_profile"
  ];
  # dot_agents/skills/** is vendored third-party skill content (same RULE as the
  # mdformat exclude below): it must stay byte-identical to upstream/live, so it
  # is never linted or reformatted. Skills authored in this repo keep their
  # scripts shellcheck-clean by hand (and exercised by test/).
  programs.shellcheck.excludes = [
    "*.tmpl"
    "dot_agents/skills/**"
  ];

  programs.shfmt.enable = true;
  # indent_size = 2 and simplify = true are the module defaults (-i 2 -s);
  # -ci (indent case labels) has no module option, so it is appended below.
  programs.shfmt.includes = [
    "*.sh"
    "*.bash"
    "dot_bash*"
    "dot_profile"
  ];
  programs.shfmt.excludes = [
    "*.tmpl"
    "dot_agents/skills/**"
  ];
  settings.formatter.shfmt.options = [ "-ci" ];

  # Markdown — mdformat reads .mdformat.toml (wrap = 105, number = false,
  # end_of_line = "lf", validate = true) from the tree root; only the GFM
  # plugin needs wiring here.
  #
  # RULE: skill files (SKILL.md and any markdown shipped alongside a skill,
  # agent, or slash-command definition) are NEVER touched by mdformat.
  # Anthropic's skill-authoring guidance treats skills as authored prose with
  # no line-wrap or formatting requirement. There is also a mechanical reason:
  # skill/agent/command files rely on `---\nkey: value\n---` YAML frontmatter,
  # which mdformat (without the mdformat-frontmatter plugin) mangles into an
  # HR + H2 heading and breaks skill discovery. docs/superpowers/ also relies
  # on YAML frontmatter (specs and plans). docs/research/2026-04-12-worktrunk.md
  # fails mdformat's strict round-trip HTML validator (validate = true); the
  # exact GFM-table construct that trips it hasn't been isolated — quarantine
  # the one file so the rest of docs/research/ stays linted.
  programs.mdformat.enable = true;
  programs.mdformat.plugins = ps: [ ps.mdformat-gfm ];
  programs.mdformat.excludes = [
    "private_dot_claude/skills/**"
    "private_dot_claude/agents/**"
    "private_dot_claude/commands/**"
    "dot_agents/**"
    "docs/superpowers/**"
    "docs/research/2026-04-12-worktrunk.md"
  ];

  # Nix — RFC 166 style (same formatter nixfmt-tree wrapped before).
  programs.nixfmt.enable = true;
  programs.nixfmt.package = pkgs.nixfmt-rfc-style;

  # TOML — dot_aerospace.toml uses user-preferred visual alignment that
  # taplo's default formatter strips; skip it so the user's style is preserved.
  programs.taplo.enable = true;
  programs.taplo.excludes = [ "dot_aerospace.toml" ];

  # GitHub Actions workflow linting.
  programs.actionlint.enable = true;

  # JSON validation. Chezmoi modify_ templates share the .json extension of
  # their target file but contain Go template directives, so jq can't parse
  # them.
  settings.formatter.jq-validate = {
    command = jqValidate;
    includes = [ "*.json" ];
    # Both spellings: treefmt matches patterns against the whole path from the
    # tree root, so bare "modify_*" only covers root-level files.
    excludes = [
      "modify_*"
      "**/modify_*"
    ];
  };

  # YAML validation — parity with lint.sh: only .chezmoidata is validated
  # (workflow YAML is covered by actionlint above).
  settings.formatter.yq-validate = {
    command = yqValidate;
    includes = [
      ".chezmoidata/*.yaml"
      ".chezmoidata/*.yml"
    ];
  };

  # Chezmoi-specific checks ported from lint.sh (see the let-bindings above).
  # `includes` is discovered programmatically (renderedShellTemplates, above):
  # every safely renderable shell template, not a hand-picked subset.
  # test/integration/rendered-template-coverage.sh guards that the discovery never silently
  # drops a template.
  settings.formatter.shellcheck-rendered-template = {
    command = shellcheckRenderedTemplate;
    includes = renderedShellTemplates;
  };
  settings.formatter.osquery-config-render = {
    command = osqueryConfigRender;
    includes = [
      ".chezmoitemplates/osquery/*.conf"
      ".chezmoitemplates/osquery/**/*.conf"
    ];
  };
}
