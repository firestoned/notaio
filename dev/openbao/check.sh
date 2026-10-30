#!/usr/bin/env bash
# Copyright (c) 2026 Erick Bourgeois, notaio
# SPDX-License-Identifier: Apache-2.0
# Check that the test OpenBao (dev/openbao/openbao.yaml, set up by setup.sh) is what ADR-0004
# asks for, from the outside, the way the signer will use it. Exits non-zero on any surprise.
#
#   - OpenBao is initialised and unsealed, and its data is in PostgreSQL
#   - transit key notaio is ed25519 and not exportable
#   - notaio-system/notaio logs in (projected token, audience openbao) and can sign
#   - ...and cannot export or rotate the key, read its config, use another key, encrypt, or
#     list policies
#   - another ServiceAccount cannot log in as role notaio
#   - a pod in another namespace cannot reach OpenBao at all
#   - the audit log records the signature request
#
# It creates what it needs (notaio-system and ServiceAccount notaio if absent, a probe
# namespace) and removes what it created.
#
#   check.sh <kubeconfig> <state dir>
set -euo pipefail

KUBECONFIG_PATH=${1:?kubeconfig}
STATE=${2:?state dir}
export KUBECONFIG=$KUBECONFIG_PATH
IMAGE=quay.io/openbao/openbao:2.7.1@sha256:6d2b93856e3fcf7b18ad855a0b51eaba474dc8b79cf554379ea32034797d2acf
OTHER_NS=openbao-check
FAILED=0
CREATED_NS=""
CREATED_SA=""
WORK=$(mktemp -d)

cleanup() {
  kubectl -n notaio-system delete pod openbao-check-notaio openbao-check-wrong-sa --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl -n notaio-system delete configmap openbao-check --ignore-not-found >/dev/null 2>&1 || true
  [ -n "$CREATED_SA" ] && kubectl -n notaio-system delete serviceaccount notaio --ignore-not-found >/dev/null 2>&1 || true
  [ -n "$CREATED_NS" ] && kubectl delete namespace notaio-system --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl delete namespace "$OTHER_NS" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

expect() { # description wanted got
  if [ "$2" = "$3" ]; then
    printf '  ok    %-44s %s\n' "$1" "$3"
  else
    printf '  FAIL  %-44s %s (wanted %s)\n' "$1" "$3" "$2"
    FAILED=1
  fi
}

ROOT=$(jq -r .root_token "$STATE/init.json")
bao_root() { kubectl -n openbao exec deploy/openbao -- env BAO_TOKEN="$ROOT" bao "$@"; }

echo "openbao"
st=$(kubectl -n openbao exec deploy/openbao -- bao status -format=json 2>/dev/null || true)
expect "initialised" true "$(jq -r .initialized <<<"$st")"
expect "unsealed" false "$(jq -r .sealed <<<"$st")"
expect "storage backend" postgresql "$(jq -r .storage_type <<<"$st")"
rows=$(kubectl -n openbao exec statefulset/openbao-postgres -- psql -U openbao -d openbao -Atc 'select count(*) > 0 from openbao_kv_store')
expect "data in PostgreSQL" t "$rows"
key=$(bao_root read -format=json transit/keys/notaio)
expect "transit key notaio type" ed25519 "$(jq -r .data.type <<<"$key")"
expect "transit key notaio exportable" false "$(jq -r .data.exportable <<<"$key")"

if ! kubectl get namespace notaio-system >/dev/null 2>&1; then
  kubectl create namespace notaio-system >/dev/null
  kubectl label namespace notaio-system pod-security.kubernetes.io/enforce=restricted >/dev/null
  CREATED_NS=1
fi
if ! kubectl -n notaio-system get serviceaccount notaio >/dev/null 2>&1; then
  kubectl -n notaio-system create serviceaccount notaio >/dev/null
  CREATED_SA=1
fi
kubectl create namespace "$OTHER_NS" >/dev/null
kubectl label namespace "$OTHER_NS" pod-security.kubernetes.io/enforce=restricted >/dev/null

cat > "$WORK/as-notaio.sh" <<'S'
r() { if "$@" >/dev/null 2>&1; then echo allowed; else echo denied; fi; }
# A brand new pod may not be admitted by the NetworkPolicy yet (the CNI adds its address to the
# allowed set asynchronously), so the first connections can be dropped. Retry, as the signer must.
login() {
  i=0
  while [ $i -lt 30 ]; do
    i=$((i + 1))
    if t=$(bao write -field=token auth/kubernetes/login role=notaio jwt=@/var/run/secrets/openbao/token 2>/dev/null); then
      echo "$t" > /tmp/token
      echo "attempts=$i"
      return 0
    fi
    sleep 2
  done
  echo "attempts=$i"
  return 1
}
{
if login > /tmp/login; then
  BAO_TOKEN=$(cat /tmp/token)
  export BAO_TOKEN
  echo "login=ok"
  cat /tmp/login
  echo "sign=$(r bao write transit/sign/notaio input=aGVsbG8=)"
  echo "export=$(r bao read transit/export/signing-key/notaio)"
  echo "rotate=$(r bao write -f transit/keys/notaio/rotate)"
  echo "read-key=$(r bao read transit/keys/notaio)"
  echo "other-key=$(r bao write transit/sign/other input=aGVsbG8=)"
  echo "encrypt=$(r bao write transit/encrypt/notaio plaintext=aGVsbG8=)"
  echo "list-policies=$(r bao policy list)"
else
  echo "login=failed"
  cat /tmp/login
fi
} > /dev/termination-log
S
cat > "$WORK/wrong-sa.sh" <<'S'
{ if bao write -field=token auth/kubernetes/login role=notaio jwt=@/var/run/secrets/openbao/token >/dev/null 2>&1; then echo "login=ok"; else echo "login=denied"; fi; } > /dev/termination-log
S
cat > "$WORK/other-ns.sh" <<'S'
{ if wget -q -T 5 -O /dev/null --no-check-certificate https://openbao.openbao.svc:8200/v1/sys/health 2>/dev/null; then echo "reach=reachable"; else echo "reach=blocked"; fi; } > /dev/termination-log
S
CA=$(kubectl -n openbao get secret openbao-tls -o jsonpath='{.data.ca\.crt}' | base64 -d)
for ns in notaio-system "$OTHER_NS"; do
  kubectl -n "$ns" create configmap openbao-check --from-file="$WORK" --from-literal=ca.crt="$CA" >/dev/null
done

probe() { # name namespace serviceaccount script
  kubectl apply -f - >/dev/null <<YAML
apiVersion: v1
kind: Pod
metadata: { name: $1, namespace: $2 }
spec:
  serviceAccountName: $3
  restartPolicy: Never
  securityContext: { runAsNonRoot: true, runAsUser: 100, seccompProfile: { type: RuntimeDefault } }
  containers:
    - name: probe
      image: $IMAGE
      command: ["sh", "/probe/$4"]
      env:
        - { name: BAO_ADDR, value: "https://openbao.openbao.svc:8200" }
        - { name: BAO_CACERT, value: "/probe/ca.crt" }
        - { name: HOME, value: "/tmp" }
      securityContext: { allowPrivilegeEscalation: false, readOnlyRootFilesystem: true, capabilities: { drop: [ALL] } }
      volumeMounts:
        - { name: token, mountPath: /var/run/secrets/openbao, readOnly: true }
        - { name: probe, mountPath: /probe, readOnly: true }
        - { name: tmp, mountPath: /tmp }
  volumes:
    - name: token
      projected: { sources: [ { serviceAccountToken: { audience: openbao, expirationSeconds: 600, path: token } } ] }
    - name: probe
      configMap: { name: openbao-check }
    - name: tmp
      emptyDir: {}
YAML
}
result() { # name namespace
  local p
  for _ in $(seq 1 90); do
    p=$(kubectl -n "$2" get pod "$1" -o jsonpath='{.status.phase}' 2>/dev/null || true)
    [ "$p" = Succeeded ] || [ "$p" = Failed ] && break
    sleep 2
  done
  kubectl -n "$2" get pod "$1" -o jsonpath='{.status.containerStatuses[0].state.terminated.message}'
}
field() { sed -n "s/^$1=//p" <<<"$2"; }

probe openbao-check-notaio notaio-system notaio as-notaio.sh
probe openbao-check-wrong-sa notaio-system default wrong-sa.sh
probe openbao-check-other "$OTHER_NS" default other-ns.sh

echo "notaio-system/notaio"
out=$(result openbao-check-notaio notaio-system)
expect "log in (role notaio)" ok "$(field login "$out")"
echo "        (logged in on attempt $(field attempts "$out"); a new pod waits for the NetworkPolicy)"
expect "sign with transit/notaio" allowed "$(field sign "$out")"
for op in export rotate read-key other-key encrypt list-policies; do
  expect "$op" denied "$(field "$op" "$out")"
done
echo "other identities"
expect "log in as another ServiceAccount" denied "$(field login "$(result openbao-check-wrong-sa notaio-system)")"
expect "reach OpenBao from another namespace" blocked "$(field reach "$(result openbao-check-other "$OTHER_NS")")"
echo "audit"
audited=$(kubectl -n openbao logs deploy/openbao --since=10m | grep -c '"path":"transit/sign/notaio"' || true)
expect "signature request in the audit log" yes "$([ "$audited" -gt 0 ] && echo yes || echo no)"

[ "$FAILED" -eq 0 ] && echo "check: all as expected" || { echo "check: FAILED"; exit 1; }
