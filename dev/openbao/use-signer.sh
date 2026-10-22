#!/usr/bin/env bash
# Switch a running notaio controller between its two signers (ADR-0004).
#
#   use-signer.sh <kubeconfig> openbao       the OpenBao transit key: ConfigMap openbao-ca from
#                                            OpenBao's CA, deploy/signer/openbao-networkpolicy.yaml,
#                                            deploy/signer/openbao-patch.yaml (which unmounts the
#                                            development seed)
#   use-signer.sh <kubeconfig> development   back to deploy/controller/20-deployment.yaml as
#                                            written, keeping the running image
#
# The OpenBao it points at is the one dev/openbao/setup.sh stands up (namespace openbao).
set -euo pipefail

KUBECONFIG_PATH=${1:?kubeconfig}
MODE=${2:?openbao or development}
NS=notaio-system
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
K=(kubectl --kubeconfig "$KUBECONFIG_PATH")

image=$("${K[@]}" -n "$NS" get deployment notaio -o jsonpath='{.spec.template.spec.containers[0].image}')

case "$MODE" in
  openbao)
    ca=$("${K[@]}" -n openbao get secret openbao-tls -o jsonpath='{.data.ca\.crt}' | base64 -d)
    "${K[@]}" -n "$NS" create configmap openbao-ca --from-literal=ca.crt="$ca" \
      --dry-run=client -o yaml | "${K[@]}" apply -f - >/dev/null
    "${K[@]}" apply -f "$ROOT/deploy/signer/openbao-networkpolicy.yaml" >/dev/null
    "${K[@]}" -n "$NS" patch deployment notaio --type strategic \
      --patch-file "$ROOT/deploy/signer/openbao-patch.yaml" >/dev/null
    "${K[@]}" -n "$NS" rollout status deployment/notaio --timeout=180s >/dev/null
    echo "notaio: signing with OpenBao transit ($image)"
    ;;
  development)
    "${K[@]}" replace -f "$ROOT/deploy/controller/20-deployment.yaml" >/dev/null
    "${K[@]}" -n "$NS" set image deployment/notaio notaio="$image" >/dev/null
    "${K[@]}" -n "$NS" delete networkpolicy notaio-openbao --ignore-not-found >/dev/null
    "${K[@]}" -n "$NS" delete configmap openbao-ca --ignore-not-found >/dev/null
    "${K[@]}" -n "$NS" rollout status deployment/notaio --timeout=180s >/dev/null
    echo "notaio: development signer restored ($image)"
    ;;
  *)
    echo "mode must be openbao or development" >&2
    exit 2
    ;;
esac
