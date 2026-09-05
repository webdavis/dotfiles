# atlas.nvim evaluated against octo.nvim

**Date:** 2026-09-04 · **Plan task:** 40a (PR 24a) · **Spec:** 5.6 · **Machine:** dresden **Subject:**
`emrearmagan/atlas.nvim` at `5aa47cd33400a137bf59df84e088a954e1add3b0` (tag 0.7.3, released 2026-08-26)
**Incumbent:** `pwntester/octo.nvim` at `09ff70efd885fe1cdf62505dce3a9bc6baeb85e1`, as pinned and
configured in `dot_config/nvim/lua/plugins/git.lua`

This pull request adds this document and nothing else. No plugin was declared through chezmoi, the lock
was not touched, and no `chezmoi apply` was run.

______________________________________________________________________

## 0. Adopted side by side with octo, 2026-09-05

Installed beside octo by operator ruling, not in place of it. The DO NOT ADOPT verdict below is left
exactly as written, because it answers the question this document actually asked, which was whether to
REPLACE octo. That is not the question the operator settled. Octo keeps `<leader>gh` and all nine of its
keymaps, its spec in `dot_config/nvim/lua/plugins/git.lua` is untouched, and section 5's cost accounting
never comes due: nothing is being given up, so the three capabilities with no atlas equivalent (gists,
standalone workflow runs, the repository list) stay reachable through octo.

Atlas is declared on its own in `dot_config/nvim/lua/plugins/atlas.lua`, pinned to the same 0.7.3 commit
measured here, lazy behind `:Atlas`, `:AtlasDiff` and a `<leader>gt` prefix. The two questions section 5
left open for an adopt pull request were answered in that file. The diff viewer is native AtlasDiff, for
the reason section 5 gives. The `pulls.repo_config.paths` mapping is one wildcard line,
`["webdavis/*"] = "~/workspaces/Ivy/webdavis/*"`; section 4.5 guessed the worktrunk tree at
`~/.herdr/worktrees/<repo>/<branch>` would be the mapping target, and that turned out to be wrong. That
directory is a plain container rather than a git repository, and each directory inside it already has a
branch checked out, so neither level is somewhere atlas can put a pull request branch.

Everything in sections 1 through 6 is the record of the evaluation and is not amended by this section.

Pull request: TODO.

______________________________________________________________________

## 1. Verdict

# DO NOT ADOPT

Keep octo. Atlas is not worse than octo at anything measured here, and it is better at a few things, but
the single capability that would justify the swap has no user on this machine, and the plugin's own
authors say it is not ready to be depended on.

**Deciding rationale.** Atlas exists to review pull requests across GitHub, GitLab and Bitbucket, and to
manage issues across GitHub, GitLab and Jira. Everything octo cannot do sits in the columns of that
matrix that are not GitHub. **Nothing found on dresden reaches those columns**, and "found" is the
load-bearing word: no GitLab token, Bitbucket credential, Jira site or `glab` binary turned up in the
four sources 4.6 lists, and nothing outside those four was searched. `gh repo list webdavis` enumerates
GitHub only, so it is not evidence either way here. The differentiator is real, and on the evidence
available it is worth nothing here today. What is left after removing it is a straight swap of one GitHub
client for another, and on that comparison the incumbent wins on three counts: atlas's README opens with
"Still in early development, will have breaking changes!" against a tag history that starts at 0.6.6 and
reaches 0.7.3, so adopting means tracking a moving target with an overhaul already in flight; atlas
refuses to check out a pull request branch until an explicit `pulls.repo_config.paths` mapping exists,
where octo checks out into the current repository with no configuration at all; and three of the nine
`<leader>gh` keymaps this config binds today have no atlas equivalent (gists, standalone workflow runs,
repository list).

**One claimed atlas advantage was withdrawn on verification.** An earlier reading of this evaluation
credited atlas with reviewing a pull request in a repository that is not checked out locally, and
credited octo with needing that checkout. Octo does not need it: `:Octo <url>` parses the hostname, the
`owner/name` pair and the number out of the address and loads through GitHub with the repository passed
explicitly, never consulting local git (verified in the pinned source, 4.2). What remains on atlas's side
of the ledger is a visible and editable query line that spans repositories by default, the local review
notes below, a native diff viewer, and three non-GitHub providers with no usable account found in the
listed sources. None of that moves the verdict, and the verdict rests on the three counts above.

Atlas does add one thing worth naming, because a sibling task is about to ask the same question from the
other side. Its local review notes attach an ISSUE, SUGGESTION, NOTE or PRAISE to a file and line without
posting anything to the forge, they survive across sessions, and they have a scriptable front end at
`bin/atlas-notes` that an agent can drive. That is the same territory PR 24b evaluates `review.nvim`
against the `herdr-nvim` annotation flow. If 24b concludes the annotation flow needs a diff-anchored
local note store, this document should be re-read before a third-party plugin is chosen, because atlas
already has one and it works.

**Reconsider when any of these becomes true.** A GitLab, Bitbucket or Jira account enters the daily
workflow, at which point the matrix atlas was built for finally has a second column and octo cannot
follow. Or atlas cuts a 1.0 and drops the breaking-changes warning, which turns "track a moving target"
into an ordinary pin. Or PR 24b lands on local diff annotations as a need, in which case atlas's notes
plus its native diff viewer is a stronger answer than adding a fourth plugin beside octo.

______________________________________________________________________

## 2. How it was tested

Atlas was installed by hand for the day, outside chezmoi, into a throwaway Neovim root at `/tmp/at` with
its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME`. That root holds a copy
of this branch's `dot_config/nvim` with the two `dot_` source names rendered, plus one extra spec file,
`lua/plugins/zz-atlas-eval.lua`, pinning atlas to the commit above. The real `~/.local/share/nvim` was
copied in with `cp -Rc` so lazy had only atlas to clone. Nothing was written to `~/.config/nvim`,
`~/.local/share/nvim`, or the branch's `dot_config/nvim`.

Every probe registers its work on a `VimEnter` autocmd and calls `os.exit` itself, and every probe
asserts `stdpath("config")` and that `atlas` is on the runtimepath before doing anything. That assertion
was mutation-verified: pointed at an empty config root it exits 1 with
`PROBE FAIL: stdpath(config)=/tmp/at/empty/nvim`. The harness root is short on purpose, because a long
one silently truncates Neovim's luac cache filenames and drops plugin modules with no error.
`GH_CONFIG_DIR` is passed at the real path, because `gh` reads its credentials out of `XDG_CONFIG_HOME`
and an isolated Neovim root would otherwise make every `gh` call look unauthorized.

Probes ran against the repositories the operator actually works in: `webdavis/dotfiles` and
`webdavis/Homelab`. GitHub was touched read-only throughout. No comment was posted, no review was
submitted, no approval or change request was filed, and no pull request was opened or modified. That was
confirmed after the fact: `webdavis/dotfiles` pull request 51 still reports 0 review comments, 0 reviews
and 0 issue comments.

`:checkhealth atlas` on the throwaway root:

```
atlas:                                                                    1 ⚠️
Requirements ~
- ✅ OK Neovim version compatible
- ✅ OK Git found: git
- ✅ OK curl found: curl
Pulls ~
- ⚠️ WARNING pulls.repo_config.paths is empty
- ✅ OK pulls.diff.open_cmd available: AtlasDiff
GitHub ~
- ✅ OK gh CLI found
- ✅ OK gh authenticated
Keymaps ~
- ✅ OK pulls: no conflicting mapped keys
```

The keymap section matters for a plugin dropped into a config with 448 normal-mode global maps: atlas
reports no conflicts, because its keys are buffer-local to its own windows.

______________________________________________________________________

## 3. Outcome table

- **List and filter pull requests: pass, both.** octo: `Octo pr list [repo] [k=v]`, bound at
  `<leader>ghp`, through the snacks picker. atlas: an `:Atlas pulls github` dashboard with GitHub search
  syntax on a visible query line, and per-view configs.
- **Read review comments: pass, both.** octo: an `Octo pr edit <n>` conversation buffer and the
  `Octo review start` thread panel. atlas: `:Atlas review <url>` loads full threads with bodies, line
  ranges, resolution state and resolver. Neither needs a local checkout (4.2).
- **Post review comments: undecided, write not exercised.** octo: `Octo review start`,
  `Octo comment add`, `Octo review submit`. atlas: `c` adds a pending comment, `C` submits it, `s` and
  `S` for suggestions, `<leader>n` for a local note.
- **Approve and request changes: undecided, write not exercised.** octo: `Octo review submit`, then the
  approve or request-changes key in the submit window. atlas: `ga` approve, `gr` request changes, `gs`
  start and submit, all gated on an open pull request.
- **Check out a pull request branch: pass, both, atlas needs setup.** octo: `Octo pr checkout`, no
  configuration, uses the current repository. atlas: `gc`, but only once an explicit
  `pulls.repo_config.paths` mapping exists.
- **Reach a non-GitHub forge: undecided, no reachable account found.** octo: not possible, zero GitLab or
  Bitbucket references in its source. atlas: GitLab, Bitbucket and Jira are first-class providers with
  full dashboards, and all three hard-require a credential. No usable account was found in the sources
  4.6 lists, and absence beyond them is unverified.

______________________________________________________________________

## 4. Row detail

### 4.1 List and filter pull requests: pass, both

`:Atlas pulls github` from `webdavis/dotfiles`, default view:

```
                                    GitHub  
  Me (1)                                         Open  Merged  Declined  |  󱅫 7
  is:pr involves:@me is:open
    Title                                󰘽     Author    󱓉          󰃭    󰥔  
   #51 docs(forzare): Bob executi..  0     󰦖   webdavis  +7189 -0   1mo  1mo
    webdavis/dotfiles
   #24 docs(osquery): v2 three-ti..  0     󰦖   webdavis  +6553 -0   2mo  2mo
    webdavis/dotfiles
   #7 Random stuff                   0   󰦖  󰦖   webdavis  +250 -145  4y   4y
    webdavis/mac-dev-playbook
```

Filtering was exercised through the keys rather than the commands behind them. Pressing `gpm` in that
buffer rewrote the query line to `is:pr involves:@me is:open OR is:pr involves:@me is:merged` and
refilled the list with #328, #327, #326, #325 and #324. Pressing `gpo` afterwards left
`is:pr involves:@me is:merged`, so the two filter keys are independent toggles on one query.

A repository-scoped view configured as
`search = "repo:webdavis/dotfiles is:pr is:merged sort:updated-desc"` produced exactly the merged pull
requests of that repository, newest first, starting at #329.

Octo covers the same ground through `Octo pr list` with `key=value` arguments and its own snacks picker,
already bound at `<leader>ghp`. Neither tool is short of anything here. Atlas's advantage is that its
query is a visible, editable line rather than a command argument, and it spans repositories by default.

### 4.2 Read review comments: pass, both

`:Atlas review https://github.com/webdavis/Homelab/pull/6` loaded 29 review comments across 12 files
without a local clone of that repository, cloning into its own cache under
`$XDG_CACHE_HOME/nvim/atlas/repos/github.com/webdavis/Homelab`. Annotated paths came back as
`.gitignore`, `README.md`, `build_charity.yml`, `devenv.yml`, `docs/ubuntu-dev-environment.md`,
`group_vars/all/main.yml`, `hosts.ini`, `roles/firmware/handlers/main.yml`,
`roles/firmware/tasks/uart.yml`, `roles/golang/tasks/install.yml`, `roles/packer/tasks/install.yml` and
`roles/packer/tasks/packer_builder_arm.yml`.

The first four comments, as atlas holds them:

````
1. README.md:3..6  state=RESOLVED outdated=true author=webdavis
    This is accurate, though it doesn't define the scope of this project.
2. README.md:nil..11  state=RESOLVED outdated=true author=webdavis
    ```suggestion ⏎ this projects development environment on an Ubuntu 20.04+ machine. ...
3. README.md:nil..11  state=RESOLVED outdated=true author=webdavis
    ```suggestion ⏎ this projects development environment, on an Ubuntu 20.04+ machine. ...
4. README.md:17..19  state=RESOLVED outdated=true author=webdavis
    Pretty sure `ubuntu-dev-environment.md` needs to be updated to include the full scope ...
````

Each comment carries `content_raw`, the inline path with a start and end line, `state`, `outdated`, the
author, `resolved_by`, the thread id and the `html_url`. Suggestion blocks survive intact. The native
diff viewer opened alongside with a file explorer, a commit list and an empty review panel, in buffers
named `atlas-diff://2/files`, `atlas-diff://2/commits` and `atlas-diff://2/review`.

Octo reads the same threads through its conversation buffer and its thread panel, and **it does not need
the repository checked out either.** Verified in the pinned source: `:Octo <url>` runs `utils.parse_url`,
whose `URL_ISSUE_PATTERN` captures the hostname, the `owner/name` pair and the number, and the
pull-request branch calls `utils.get_pull_request(number, repo)` with that repo passed explicitly.
`uri.get_repo_id_from_args` consults `utils.get_remote_name()` only on the one-argument form; given two
arguments it takes `args[2]` and never touches local git. So a full pull-request address opens against
GitHub from anywhere, and this row is a genuine tie on reading.

### 4.3 Post review comments: undecided, write not exercised

Not performed, per the read-only constraint on this evaluation. The command path was mapped instead.

The GitHub provider advertises `add_comment`, `edit_comment`, `delete_comment`, `set_thread_resolved`,
`add_reaction`, `fetch_conversation`, `reaction_options` and `comment_completion` as comment
capabilities. The keys resolve to `c` for `pulls.review.diff.add_comment`, `C` for
`pulls.review.diff.submit_comment`, `s` and `S` for the suggestion pair, and `<leader>n` for
`pulls.review.diff.add_note`.

There is no dry-run or preview on the posting path, and the reason is worth recording. Atlas follows
GitHub's pending-review model: `c` adds a comment to a pending review, which means an
`addPullRequestReviewThread` mutation reaches GitHub at that moment even though nobody else can see it
yet, and `gs` submits the batch. So the first keystroke is already a write. The only genuinely local
option is `<leader>n`, and that is a separate feature rather than a preview of the first.

Local notes were exercised, because they touch nothing but disk:

```
$ ./bin/atlas-notes add --target https://github.com/webdavis/dotfiles/pull/51 \
    --file docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md --line 1 \
    --context "# Neovim overhaul plan" --type note --body "atlas.nvim evaluation probe, local only"
{"line":1,"body":"atlas.nvim evaluation probe, local only","created_at":"2026-09-05T01:02:53Z",
 "id":"note_2493867e20eeee57","file_path":"docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md",
 "type":"note","context":{"lines":["# Neovim overhaul plan"],"start_line":1}}
```

The note landed in `$XDG_DATA_HOME/nvim/atlas/notes` and reads back through
`atlas-notes list --target <url>`. Nothing reached GitHub: pull request 51 still shows 0 review comments
and 0 reviews. Octo has no equivalent; every octo comment path posts.

### 4.4 Approve and request changes: undecided, write not exercised

Not performed. The availability gate was read instead, on both a closed and an open pull request, which
proves the path exists and is correctly refused where it should be.

On `webdavis/Homelab` pull request 6, which is merged:

```
pr.state=merged
is_available(approve)=false
is_available(request_changes)=false
is_available(submit_review)=false
is_available(merge)=false
```

On `webdavis/dotfiles` pull request 51, which is open:

```
pr #51 state=open   pending review = false
capabilities.reviews = { approve, discard_review, edit_review, fetch, fetch_review_context,
                         request_changes, set_file_reviewed, start_review, submit_review }
is_available(approve)=true
is_available(request_changes)=true
is_available(submit_review)=false
is_available(merge)=true
keymap pulls.review.approve         -> { "ga" }
keymap pulls.review.request_changes -> { "gr" }
keymap pulls.review.submit_review   -> { "gs" }
```

`submit_review` reads false on an open pull request because no pending review exists yet; it becomes
available after `start_review`. So both approve and request changes are one keystroke from a live review
session, with a body prompt, and neither offers a confirmation step or a preview. Octo puts the same two
actions behind its submit window, which does show the pending comments before sending.

### 4.5 Check out a pull request branch: pass, both, atlas needs setup

Atlas will not check anything out until `pulls.repo_config.paths` maps the repository to a local clone.
`resolve_repo_path_for_pr` reads that mapping and nothing else, with no fallback to the current working
directory, which is why `:checkhealth atlas` warns about the empty mapping out of the box.

With `{ ["webdavis/*"] = "/tmp/at/co/*" }` and a fresh clone at `/tmp/at/co/dotfiles`:

```
pr #51 docs(forzare): Bob executive-assistant design spec + implementation plan
       state=open head=docs/forzare-plan-spec-hardening
branch before: main
checkout done=true result={ local_branch = "docs/forzare-plan-spec-hardening",
                            repo_path = "/private/tmp/at/co/dotfiles" } err=nil
branch after: docs/forzare-plan-spec-hardening
```

The branch did not exist locally, so atlas fetched the pull request refs and created it. This is a local
git operation; nothing was written to GitHub.

`Octo pr checkout` does the same thing in the repository the buffer belongs to, with nothing to
configure. For a single-machine, single-clone-per-repo setup the atlas mapping is pure overhead. It pays
off only when one machine holds many clones under a predictable layout, which is exactly what
`worktree-path` at `~/.herdr/worktrees/<repo>/<branch>` is, so the mapping would be one wildcard line if
this were adopted.

### 4.6 Reach a non-GitHub forge: undecided, no reachable account

This is the row atlas exists for, and it is the row that cannot be settled on this machine.

Octo's side is settled and it is a hard no. A recursive search of `octo.nvim/lua` for `gitlab` or
`bitbucket` returns zero files. Octo speaks to `gh` and nothing else.

Atlas registers four providers:

```
bitbucket(pulls=true,issues=false), github(pulls=true,issues=true),
gitlab(pulls=true,issues=true),    jira(pulls=false,issues=true)
```

Declaring `providers.gitlab = { base_url = "https://gitlab.com" }` immediately adds `gitlab` to the
configured pull-request providers, and `:Atlas pulls gitlab` opens a fully built dashboard with its own
views:

```
                                    GitLab  
  Assigned (1)    Created (2)                      Open  Merged  Declined  |  󰂚
  is:open scope:assigned_to_me
Error: Missing GitLab credentials in config
```

The user interface is real rather than a stub, and it fails cleanly at the credential boundary rather
than crashing. It cannot get past that boundary without a token: `get_auth` in the GitLab client returns
an error whenever `base_url` or `token` is empty, so there is no unauthenticated read path even against a
public gitlab.com project. Bitbucket and Jira are the same shape, refusing with "Missing Bitbucket
credentials in config (providers.bitbucket.user / providers.bitbucket.token)" and "Missing Jira
credentials in config".

**No usable account was found in the sources listed below, and absence is not verified beyond them.**
What was checked, and found empty: the environment for any `GITLAB_*`, `BITBUCKET_*` or `JIRA_*`
variable; `~/.config` for a `glab`, Bitbucket or Jira directory; `PATH` for a `glab` binary; and
`.chezmoidata/system_packages_autoinstall.yaml` for any GitLab or Bitbucket package. A fifth check, that
every repository in `gh repo list webdavis` is on github.com, proves nothing about the question: `gh`
enumerates GitHub and only GitHub, so it cannot report a GitLab or Bitbucket repository whether one
exists or not. A credential, a Jira site or a non-GitHub repository held anywhere outside those four
sources would not have been seen. Creating an account or minting a token would be a write against the
operator's identity, which is outside this evaluation.

So: atlas claims GitHub, GitLab, Bitbucket and Jira; the GitLab surface is demonstrably wired end to end
up to the token check; and whether it actually works against a live GitLab or Bitbucket instance is
untested and untestable here.

______________________________________________________________________

## 5. What adopting would cost

An adopt outcome becomes its own pull request. It would drop the octo block at
`dot_config/nvim/lua/plugins/git.lua:1194-1256`, which is the spec, its three dependencies, its nine
`<leader>gh` keymaps, and the `FileType octo` autocmd that registers eight `<localleader>` groups per
buffer.

Those eight groups are the concrete cost surface. They are which-key metadata over octo's own
buffer-local keymaps, so they are deleted rather than ported, but each one names a capability that has to
land somewhere in atlas or be given up:

- **`<localleader>a` Assignee.** octo: `aa` add, `ad` remove. atlas: the `edit_assignees` action on the
  pull request.
- **`<localleader>c` Comment.** octo: `ca` add, `cr` reply, `cd` delete. atlas: `a` and `i` add, `c`
  reply, `e` edit, `dd` delete.
- **`<localleader>i` Issue.** octo: `ic` close, `io` reopen, `il` list. atlas: `:Atlas issues github`,
  `create_issue`, `reopen`.
- **`<localleader>g` Navigate.** octo: `gi` go to a repository issue. atlas: `:Atlas open <target>`.
- **`<localleader>l` Label.** octo: `lc` create, `la` add, `ld` remove. atlas: the `labels` action,
  backed by `list_labels` and `update_labels`.
- **`<localleader>p` PR.** octo: `po` checkout, `pm`, `psm` and `prm` merge, `pc` commits, `pf` files,
  `pd` diff. atlas: `gc` checkout, the `merge` action, `gC` commits, the file explorer and AtlasDiff.
- **`<localleader>r` React.** octo: eight reaction keys, `rt` and `rT` resolve and unresolve threads.
  atlas: `gr` react with `reaction_options`, `x` toggle resolved.
- **`<localleader>v` Review.** octo: `vs` start and submit, `vd` discard, `va` and `vd` reviewers. atlas:
  `gs`, `ga`, `gr`, `discard_review`, `edit_reviewers`.

Every row has a home. The three that do not are outside those groups, in the `<leader>gh` keymaps:

- `<leader>ghg` `Octo gist list`. Atlas has no gist support at all.
- `<leader>ghw` `Octo run list`. Atlas reads GitHub Actions runs only as pipelines attached to a pull
  request, through `open_pipelines`; there is no standalone command for a repository's runs.
- `<leader>ghr` `Octo repo list`. Atlas fetches repository detail and branches as pull-request context,
  and has no repository browser.

`<leader>ghn` (notifications) does survive: atlas has a notifications surface for GitHub and GitLab, on
`N`.

Section 8.2's rule for new keymaps would also apply to whatever replaces them, and atlas would want a
`<leader>` prefix of its own for `:Atlas pulls`, `:Atlas issues`, `:Atlas review` and `:Atlas notes`,
plus a group row in `lua/plugins/which-key.lua`. Its in-buffer keys need no which-key work: they are
buffer-local, they collide with nothing (`:checkhealth atlas` confirms), and atlas ships its own help
popup on `g?`.

Two further pieces of work an adopt pull request would own. It has to decide the diff viewer, since
AtlasDiff is native but the alternatives (codediff, diffview) are documented as relying on plugin
internals that break on upstream changes, so native is the only defensible pick. And it has to add the
`pulls.repo_config.paths` mapping, one wildcard line pointing at the worktrunk layout, or checkout stays
broken.

______________________________________________________________________

## 6. Cleanup

The throwaway tree at `/tmp/at` was trashed at the end of the evaluation, along with the scratch clone at
`/tmp/at/co/dotfiles` and the atlas source checkout used to read its code. Nothing under `~/.config/nvim`
or `~/.local/share/nvim` was written to at any point; both were read-only inputs, the second only as the
source of a `cp -Rc`.
