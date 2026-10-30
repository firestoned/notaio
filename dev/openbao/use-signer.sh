#!/usr/bin/env bash
# Copyright (c) 2026 Erick Bourgeois, notaio
# SPDX-License-Identifier: Apache-2.0
# Prepare or switch the signer of a notaio controller (ADR-0004).
#
#   use-signer.sh <kubeconfig> ca            ConfigMap openbao-ca in notaio-system from the CA of
#                                            the dev/openbao OpenBao: the default deployment needs
#                                            it before its pod can start
#   use-signer.sh <kubeconfig> openbao       the default: deploy/controller/20-deployment.yaml as
#                                            written (OpenBao transit), keeping the running image
#   use-signer.sh <kubeconfig> development   the opt-in development seed for local experiments:
#                                            Secret notaio-dev-signing-key and
#                                            deploy/signer/development-patch.yaml
set -euo pipefail

KUBECONFIG_PATH=${1:?kubeconfig}
MODE=${2:?ca, openbao or development}
NS=notaio-system
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
K=(kubectl --kubeconfig "$KUBECONFIG_PATH")

ca_configmap() {
  local ca
  ca=$("${K[@]}" -n openbao get secret openbao-tls -o jsonpath='{.data.ca\.crt}' | base64 -d)
  [ -n "$ca" ] || { echo "no CA in Secret openbao/openbao-tls: run dev/openbao/setup.sh first" >&2; exit 1; }
  "${K[@]}" -n "$NS" create configmap openbao-ca --from-literal=ca.crt="$ca" \
    --dry-run=client -o yaml | "${K[@]}" apply -f - >/dev/null
}

current_image() {
  "${K[@]}" -n "$NS" get deployment notaio -o jsonpath='{.spec.template.spec.containers[0].image}'
}

case "$MODE" in
  ca)
    ca_configmap
    echo "notaio: ConfigMap openbao-ca from OpenBao's CA"
    ;;
  openbao)
    ca_configmap
    image=$(current_image)
    "${K[@]}" replace -f "$ROOT/deploy/controller/20-deployment.yaml" >/dev/null
    "${K[@]}" -n "$NS" set image deployment/notaio notaio="$image" >/dev/null
    "${K[@]}" -n "$NS" rollout status deployment/notaio --timeout=180s >/dev/null
    "${K[@]}" -n "$NS" delete secret notaio-dev-signing-key --ignore-not-found >/dev/null
    echo "notaio: signing with OpenBao transit ($image)"
    ;;
  development)
    image=$(current_image)
    "${K[@]}" -n "$NS" get secret notaio-dev-signing-key >/dev/null 2>&1 || \
      "${K[@]}" -n "$NS" create secret generic notaio-dev-signing-key \
        --from-literal=seed="$(head -c 32 /dev/urandom | base64)" >/dev/null
    "${K[@]}" -n "$NS" patch deployment notaio --type strategic \
      --patch-file "$ROOT/deploy/signer/development-patch.yaml" >/dev/null
    "${K[@]}" -n "$NS" rollout status deployment/notaio --timeout=180s >/dev/null
    echo "notaio: signing with a development seed ($image); never in production"
    ;;
  *)
    echo "mode must be ca, openbao or development" >&2
    exit 2
    ;;
esac
