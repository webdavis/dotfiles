# Hermes `config.yaml` — Track It, or Not? (Research)

**Date:** 2026-06-09 · **For:** the osquery-alerting reshape plan, Task 10 (chezmoi-tracking Hermes files).
**Question:** Does Hermes Agent (NousResearch/hermes-agent) recommend version-controlling `~/.hermes/config.yaml`
or against it, and how should secrets be kept out of it?
**Method:** the installed Hermes source + `website/docs/**` on Dresden (the exact version in use) as the
primary source; GitHub Issues/Discussions + the hosted docs as triangulation.

## Verdict

Hermes gives **no explicit for/against rule** on committing `config.yaml`, but its design dictates a clear,
safe pattern:

1. **Secrets never belong in `config.yaml`.** Hermes's hard convention is **secrets → `~/.hermes/.env`**
   (git-ignored by Hermes's own `.gitignore`), **non-secret settings → `config.yaml`**, which references
   secrets via **`${VAR}` expansion**. `hermes config set` even auto-routes API keys to `.env` and
   everything else to `config.yaml`.
2. **`config.yaml` is runtime-mutable, and that's the real hazard.** The daemon/CLI rewrite it
   (`hermes config set`, slash-commands, `migrate_config`), and **open bug #4775** reports a rewrite can
   expand defaults *and resolve `${VAR}` placeholders into plaintext secrets*. A continuously chezmoi-synced
   `config.yaml` would therefore (a) drift-war with Hermes and (b) risk a plaintext-secret leak into git.
3. **Tracking it is not prohibited** — the docs even endorse checking non-secret bundle YAML into a shared
   dotfiles repo and symlinking it in.

**→ Best fit for this repo:** track `config.yaml` as a chezmoi **`create_` template** (written once on a
fresh host, then never re-synced) containing **only `${VAR}` refs**, with the secret values in a
chezmoi-managed **`.env`** (KeePassXC). This honors "track it / reproducible baseline," matches Hermes's own
`.env`+`${VAR}` convention, and sidesteps both the drift-war and the #4775 plaintext leak (chezmoi writes
the file once and never reads it back).

## Findings (cited)

- **Secrets/settings split is the core rule.** "Secrets (API keys, bot tokens, passwords) go in `.env`.
  Everything else … goes in `config.yaml`." AGENTS.md: "config.yaml (settings), .env (API keys only)";
  ".env variables (SECRETS ONLY — API keys, tokens, passwords)."
  [`website/docs/user-guide/configuration.md`, `AGENTS.md`]
- **`${VAR}` expansion is supported in config.yaml** so secrets are *referenced, not stored*: "You can
  reference environment variables in `config.yaml` using `${VAR_NAME}` syntax." (Bare `$VAR` is not
  expanded; undefined vars stay literal.) [`configuration.md`]
- **`hermes config set` routes secrets→`.env`, settings→`config.yaml`** automatically.
  [`configuration.md:42-46`, `website/docs/reference/environment-variables.md`]
- **OPEN bug #4775 — the rewrite/resolve hazard.** "Hermes rewrites raw config.yaml with expanded defaults
  and resolved env secrets" … "can silently rewrite user-authored `~/.hermes/config.yaml`" and "replace raw
  placeholders like `${GLM_API_KEY}` with resolved secret values." Cause: save paths `load_config()` →
  mutate → `save_config()`. Still open. [`github.com/NousResearch/hermes-agent/issues/4775`]
- **Dotfiling non-secret YAML is endorsed.** "check the bundle YAML into a shared dotfiles repo and
  symlinking it into `~/.hermes/…`." [`website/docs/user-guide/features/skills.md`]
- **Keep-secrets-out rationale (community).** Issue #11239 (env-backed `${VAR}` refs): "Keeps long-lived
  secrets in `~/.hermes/.env` … instead of `config.yaml`" and "Reduces accidental secret disclosure when
  sharing config." [`issues/11239`]
- **A canonical template exists; hand-editing is expected.** `cli-config.yaml.example` header: "Copy this
  file … and customize"; inline-secret lines are commented with SECURITY warnings ("plaintext"). No
  "don't hand-edit" prohibition on `config.yaml`. [`cli-config.yaml.example`]

## Recommendation for the plan (Task 10)

- **Secrets → `~/.hermes/.env`** (`dot_hermes/private_dot_env.tmpl`, KeePassXC) — Hermes's own pattern.
- **`config.yaml` → `dot_hermes/create_private_config.yaml.tmpl`** — chezmoi `create_` writes it only if the
  target is absent and never overwrites an existing one, so Hermes keeps full runtime ownership; inline
  secrets replaced with `${VAR}`.
- Reconcile the two webhook secrets first (they differ — see Task 10 Step 1); value-based leak gate as a backstop.
- **Never** `chezmoi add` the live `config.yaml` afterward (per #4775 it may hold a resolved plaintext secret).
- Drop `config.yaml` from the FIM hash-set (runtime-mutated → noise).

## Caveat

Per-machine drift is unavoidable — Hermes mutates each host's `config.yaml` independently, so `create_`
gives a consistent *baseline*, not lockstep fleet config. That is the best achievable for an app-owned,
runtime-rewritten file.

## Sources

- `~/.hermes/hermes-agent/website/docs/user-guide/configuration.md` — secrets/settings split, `${VAR}` syntax, `hermes config set` routing
- `~/.hermes/hermes-agent/AGENTS.md` — "config.yaml (settings), .env (API keys only)"
- `~/.hermes/hermes-agent/website/docs/user-guide/features/skills.md` — dotfiles endorsement
- `~/.hermes/hermes-agent/website/docs/user-guide/features/web-dashboard.md:352` — "`.env` is for API keys/secrets only; config.yaml is the recommended place to set non-secret values"
- `github.com/NousResearch/hermes-agent/issues/4775` — OPEN: daemon rewrites config.yaml, resolves `${VAR}`→plaintext
- `github.com/NousResearch/hermes-agent/issues/11239` — OPEN: env-backed secret refs; keep secrets out of config.yaml
- `~/.hermes/hermes-agent/hermes_cli/config.py` — `save_config()`/`migrate_config()` via `atomic_yaml_write`; `.env` path helpers; RLock around load/save (runtime writes)
- chezmoi `create_` attribute — "create the file if it does not exist, otherwise leave it unchanged"
