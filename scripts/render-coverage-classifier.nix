# Pure (builtins-only) classifier for the rendered-template coverage discovery.
#
# This is the SINGLE source of truth for "which shell templates can render
# headless and are therefore safe to hand to the shellcheck-rendered-template
# formatter". treefmt.nix imports it to build that formatter's include list, and
# test/rendered-template-coverage.sh drives the SAME functions through `nix eval`
# against the fixture matrix (beside a bash mirror), asserting agreement case by
# case. Before this file existed the Nix predicates were unexported let-bindings,
# so weakening one left every fixture green; now a weakened predicate here fails
# the fixture matrix directly.
#
# No `lib`, no `pkgs`: everything is `builtins`, so the test can import it with a
# bare `nix eval --impure` and no flake machinery. Every helper is textually
# mirrored by a bash function in the test; keep the two in lockstep.
rec {
  # File split into lines (drop the capture sublists builtins.split emits).
  fileLines = path: builtins.filter builtins.isString (builtins.split "\n" (builtins.readFile path));

  # A line invokes keepassxc when the identifier keepassxc/keepassxcAttribute
  # appears ANYWHERE inside a Go-template action `{{ ... }}`, not only as the
  # first token: `{{ $e := keepassxc "x" }}` counts. A Go-template COMMENT
  # (`{{/* ... */}}`, in any trim form) does not, and a bare shell `#` comment
  # that merely mentions keepassxc does not (it carries no `{{`). Actions in this
  # repo are single-line, so the scan is per line.
  lineCallsKeepassxc =
    line:
    builtins.match ".*[{][{][^}]*keepassxc.*" line != null
    && builtins.match "[[:space:]]*[{][{]-?[[:space:]]*/[*].*" line == null;

  directCallsKeepassxc = path: builtins.any lineCallsKeepassxc (fileLines path);

  # Parse a single line for an includeTemplate directive `{{ includeTemplate <arg> ... }}`.
  # Returns null (no directive), { name = "<literal>"; } for a double-quoted or
  # backtick raw-string name, or { dynamic = true; } when the name argument is
  # not a static string literal (a $var, (expr), or .field) and so cannot be
  # resolved at eval time. Anchored on `{{` so prose mentioning includeTemplate
  # inside a comment body (which carries no `{{` on that line) is not a directive.
  parseIncludeLine =
    line:
    let
      m = builtins.match ".*[{][{]-?[[:space:]]*includeTemplate[[:space:]]+(.*)" line;
    in
    if m == null then
      null
    else
      let
        rest = builtins.head m;
        dq = builtins.match "\"([^\"]*)\".*" rest;
        bt = builtins.match "`([^`]*)`.*" rest;
      in
      if dq != null then
        { name = builtins.head dq; }
      else if bt != null then
        { name = builtins.head bt; }
      else
        { dynamic = true; };

  # All includeTemplate directives in a file: { dynamic = bool; names = [str]; }.
  includeDirectives =
    path:
    let
      hits = builtins.filter (x: x != null) (map parseIncludeLine (fileLines path));
    in
    {
      dynamic = builtins.any (h: h ? dynamic) hits;
      names = map (h: h.name) (builtins.filter (h: h ? name) hits);
    };

  # A template cannot render headless (is UNSAFE) when it, or any partial it
  # includeTemplates (transitively, literal names resolved against includeBase),
  # calls keepassxc, OR when any includeTemplate name cannot be resolved
  # statically (conservative rejection). Cycle-protected: a partial already on
  # the current include chain contributes no new unsafety, so a cyclic pair
  # terminates. A literal include whose partial is absent under includeBase is
  # skipped (it would fail at real render time, a separate concern).
  rendersUnsafe =
    includeBase: path:
    let
      go =
        visited: p:
        if builtins.elem p visited then
          false
        else if directCallsKeepassxc p then
          true
        else
          let
            incs = includeDirectives p;
          in
          if incs.dynamic then
            true
          else
            builtins.any (
              name:
              let
                partial = includeBase + "/${name}";
              in
              builtins.pathExists partial && go (visited ++ [ p ]) partial
            ) incs.names;
    in
    go [ ] path;

  # A template is a shell template when its first line is a shell shebang (or a
  # `# shellcheck shell=` directive), OR its first line is a Go-template
  # directive and its first non-directive line is such a shebang (the
  # osquery-loader shape).
  isShellShebangLine =
    line: builtins.match "#!.*sh.*" line != null || builtins.match "# shellcheck shell=.*" line != null;
  isGoDirectiveLine = line: builtins.match "[[:space:]]*[{][{].*" line != null;
  isShellTemplate =
    path:
    let
      ls = fileLines path;
      first = if ls == [ ] then "" else builtins.head ls;
      nonDirective = builtins.filter (l: !(isGoDirectiveLine l)) ls;
      firstNonDirective = if nonDirective == [ ] then "" else builtins.head nonDirective;
    in
    isShellShebangLine first || (isGoDirectiveLine first && isShellShebangLine firstNonDirective);

  isShTmpl = path: builtins.match ".*\\.sh\\.tmpl" (builtins.baseNameOf path) != null;

  # The coverage verdict for a template: "covered" when it is a shell template
  # (a *.sh.tmpl, or shell-classified by shape) that can render headless;
  # "excluded" otherwise. Mirrors the bash fixture oracle in the test.
  classify =
    includeBase: path:
    if rendersUnsafe includeBase path then
      "excluded"
    else if isShTmpl path || isShellTemplate path then
      "covered"
    else
      "excluded";
}
