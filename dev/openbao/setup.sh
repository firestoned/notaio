#!/usr/bin/env bash
# Bring dev/openbao/openbao.yaml to the state notaio's transit signer needs (ADR-0004), and
# again after any restart (it re-seals). Idempotent: every step checks before it acts.
#
#   1. apply the manifests; create Secret openbao-postgres (random password and the
#      sslmode=verify-full connection URL) the first time only
#   2. wait for the certificates, PostgreSQL and OpenBao
#   3. initialise (one Shamir share) and unseal
#   4. check the audit device (declared in openbao.hcl) is active: every signature request is
#      logged outside notaio
#   5. transit: a non-exportable ed25519 key `notaio`
#   6. Kubernetes auth: policy notaio-sign (update on transit/sign/notaio, nothing else) and a
#      role for notaio-system/notaio, audience openbao, 15 minute tokens
#   7. print the public key consumers trust (key id notaio:v<N>)
#
# The unseal key and root token are written to <state dir>/init.json, mode 600, on the machine
# that runs this. They never go into the repository or the cluster.
#
#   setup.sh <kubeconfig> <state dir>
set -euo pipefail

KUBECONFIG_PATH=${1:?kubeconfig}
STATE=${2:?state dir}
NS=openbao
HERE=$(cd "$(dirname "$0")" && pwd)
K=(kubectl --kubeconfig "$KUBECONFIG_PATH" -n "$NS")
umask 077
mkdir -p "$STATE"
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }

kubectl --kubeconfig "$KUBECONFIG_PATH" apply -f "$HERE/openbao.yaml" >/dev/null
echo "openbao: manifests applied"

if ! "${K[@]}" get secret openbao-postgres >/dev/null 2>&1; then
  PW=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
  URL="postgres://openbao:${PW}@openbao-postgres.openbao.svc:5432/openbao?sslmode=verify-full&sslrootcert=/pg-ca/ca.crt"
  "${K[@]}" create secret generic openbao-postgres \
    --from-literal=password="$PW" --from-literal=connection-url="$URL" >/dev/null
  unset PW URL
  echo "openbao: generated the PostgreSQL password"
fi

"${K[@]}" wait --for=condition=Ready certificate/openbao-tls certificate/openbao-postgres-tls --timeout=180s >/dev/null
"${K[@]}" rollout status statefulset/openbao-postgres --timeout=300s >/dev/null
"${K[@]}" rollout status deployment/openbao --timeout=300s >/dev/null
echo "openbao: PostgreSQL and OpenBao are up"

bao() { "${K[@]}" exec deploy/openbao -- bao "$@"; }
# `bao status` exits 2 while sealed and still prints its JSON; use the JSON when there is one.
status() {
  local out
  out=$(bao status -format=json 2>/dev/null) || true
  if [ -n "$out" ]; then echo "$out"; else echo '{"initialized":false,"sealed":true}'; fi
}

if [ "$(status | jq -r .initialized)" != "true" ]; then
  bao operator init -key-shares=1 -key-threshold=1 -format=json > "$STATE/init.json"
  echo "openbao: initialised (unseal key and root token in $STATE/init.json)"
fi
if [ "$(status | jq -r .sealed)" = "true" ]; then
  bao operator unseal "$(jq -r '.unseal_keys_b64[0]' "$STATE/init.json")" >/dev/null
  echo "openbao: unsealed"
fi
ROOT=$(jq -r .root_token "$STATE/init.json")
bao_root() { "${K[@]}" exec deploy/openbao -- env BAO_TOKEN="$ROOT" bao "$@"; }
bao_root_stdin() { "${K[@]}" exec -i deploy/openbao -- env BAO_TOKEN="$ROOT" bao "$@"; }

# Declared in openbao.hcl (OpenBao 2.x refuses API-created audit devices); check it is active.
bao_root audit list -format=json | jq -e '."to-stdout/"' >/dev/null 2>&1 \
  || { echo "openbao: audit device to-stdout is not active; restart deploy/openbao to load the config" >&2; exit 1; }
echo "openbao: audit device on (stdout)"

bao_root secrets list -format=json | jq -e '."transit/"' >/dev/null 2>&1 \
  || bao_root secrets enable transit >/dev/null
bao_root read transit/keys/notaio >/dev/null 2>&1 \
  || bao_root write -f transit/keys/notaio type=ed25519 exportable=false >/dev/null
bao_root read -format=json transit/keys/notaio \
  | jq -e '.data.type == "ed25519" and .data.exportable == false' >/dev/null \
  || { echo "openbao: transit key notaio is not a non-exportable ed25519 key" >&2; exit 1; }
echo "openbao: transit key notaio (ed25519, not exportable)"

bao_root auth list -format=json | jq -e '."kubernetes/"' >/dev/null 2>&1 \
  || bao_root auth enable kubernetes >/dev/null
# In-cluster: OpenBao reviews tokens with its own ServiceAccount (system:auth-delegator).
bao_root write auth/kubernetes/config kubernetes_host=https://kubernetes.default.svc:443 >/dev/null
bao_root_stdin policy write notaio-sign - >/dev/null <<'HCL'
# notaio may request signatures from its key and do nothing else (ADR-0004, decision 2).
path "transit/sign/notaio" { capabilities = ["update"] }
HCL
bao_root write auth/kubernetes/role/notaio \
  bound_service_account_names=notaio \
  bound_service_account_namespaces=notaio-system \
  audience=openbao \
  token_policies=notaio-sign \
  token_ttl=15m token_max_ttl=15m >/dev/null
echo "openbao: policy notaio-sign, Kubernetes auth role notaio for notaio-system/notaio (audience openbao)"

bao_root read -format=json transit/keys/notaio \
  | jq -r '.data.keys | to_entries | max_by(.key | tonumber) | "notaio:v\(.key) \(.value.public_key)"' \
  > "$STATE/notaio-public-key.txt"
echo "openbao: consumers trust key $(cut -d' ' -f1 "$STATE/notaio-public-key.txt"), public key in $STATE/notaio-public-key.txt"
