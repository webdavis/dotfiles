#!/usr/bin/env bash
# relay-hermes-route.yq: ensures the webhook base (enabled + host/port defaults) and sets routes.relay
# from $SECRET/$CHATID; preserves osquery + everything else; idempotent; self-sufficient (works on an
# empty or no-webhook config too -- no dependency on `hermes setup` having run first).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expr_file="$root/private/relay-hermes-route.yq"
[[ -f $expr_file ]] || {
  echo "relay-hermes-route: FAIL -- missing $expr_file" >&2
  exit 1
}
expr="$(cat "$expr_file")"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fail() {
  echo "relay-hermes-route: FAIL -- $1" >&2
  exit 1
}

# 1. existing config (webhook + osquery) -> relay merged, osquery + base preserved
cat >"$tmp/has.yml" <<'YAML'
platforms:
  webhook:
    enabled: true
    extra:
      host: 127.0.0.1
      port: 8644
      routes:
        osquery:
          secret: OSQSECRET
          deliver: discord
YAML
got="$(SECRET=SEKRET CHATID=999888 yq "$expr" "$tmp/has.yml")"
yq -e '.platforms.webhook.extra.routes.relay.secret == "SEKRET"' <<<"$got" >/dev/null 2>&1 || fail "relay secret not set from env"
yq -e '.platforms.webhook.extra.routes.relay.deliver_extra.chat_id == "999888"' <<<"$got" >/dev/null 2>&1 || fail "chat_id not set from env"
yq -e '.platforms.webhook.extra.routes | has("osquery")' <<<"$got" >/dev/null 2>&1 || fail "osquery route not preserved"
yq -e '.platforms.webhook.extra.routes.osquery.secret == "OSQSECRET"' <<<"$got" >/dev/null 2>&1 || fail "osquery content not preserved"
yq -e '.platforms.webhook.extra.port == 8644' <<<"$got" >/dev/null 2>&1 || fail "existing port not preserved"

# 2. config WITHOUT webhook -> CREATES the base (enabled+host+port) + relay; preserves the rest
cat >"$tmp/none.yml" <<'YAML'
platforms:
  model:
    name: foo
YAML
got2="$(SECRET=X CHATID=Y yq "$expr" "$tmp/none.yml")"
yq -e '.platforms.webhook.enabled == true' <<<"$got2" >/dev/null 2>&1 || fail "webhook.enabled not ensured on a config without it"
yq -e '.platforms.webhook.extra.port == 8644' <<<"$got2" >/dev/null 2>&1 || fail "default port not created"
yq -e '.platforms.webhook.extra.host == "127.0.0.1"' <<<"$got2" >/dev/null 2>&1 || fail "default host not created"
yq -e '.platforms.webhook.extra.routes.relay.secret == "X"' <<<"$got2" >/dev/null 2>&1 || fail "relay not created"
yq -e '.platforms.model.name == "foo"' <<<"$got2" >/dev/null 2>&1 || fail "non-webhook content not preserved"

# 3. EMPTY input -> creates the full webhook base + relay (no existing config required)
got3="$(printf '' | SECRET=X CHATID=Y yq "$expr")"
yq -e '.platforms.webhook.enabled == true' <<<"$got3" >/dev/null 2>&1 || fail "base not created from empty config"
yq -e '.platforms.webhook.extra.routes.relay.secret == "X"' <<<"$got3" >/dev/null 2>&1 || fail "relay not created from empty config"

# 4. idempotent: re-applying yields exactly one relay route, same content
got4="$(SECRET=SEKRET CHATID=999888 yq "$expr" <<<"$got")"
yq -e '[.platforms.webhook.extra.routes.relay] | length == 1' <<<"$got4" >/dev/null 2>&1 || fail "not idempotent"
yq -e '.platforms.webhook.extra.routes.relay.deliver_extra.chat_id == "999888"' <<<"$got4" >/dev/null 2>&1 || fail "idempotent re-apply changed chat_id"
echo "relay-hermes-route: OK"
