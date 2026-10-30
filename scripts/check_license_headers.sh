#!/usr/bin/env bash
# Copyright (c) 2026 Erick Bourgeois, notaio
# SPDX-License-Identifier: Apache-2.0
#
# Every tracked source file carries the SPDX identifier of the project's licence (LICENSE) in its
# first 10 lines: Rust, shell, Python, the Makefile, the Dockerfile and the GitHub workflows.
# Generated output (deploy/crds, deploy/profiles, docs/src/reference) and Markdown are not checked.
#
#   check_license_headers.sh            every tracked file of those kinds
#   check_license_headers.sh FILE...    just these, for testing the check itself
set -euo pipefail

want='SPDX-License-Identifier: Apache-2.0'
if [ "$#" -gt 0 ]; then
  files=$(printf '%s\n' "$@")
else
  files=$(git ls-files | grep -E '\.(rs|sh|py)$|(^|/)Makefile$|(^|/)Dockerfile$|^\.github/.*\.ya?ml$')
fi

checked=0
missing=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  checked=$((checked + 1))
  if ! head -n 10 "$f" | grep -qF "$want"; then
    echo "license-check: no '$want' in the first 10 lines of $f" >&2
    missing=$((missing + 1))
  fi
done <<< "$files"

if [ "$missing" -gt 0 ]; then
  echo "license-check: $missing of $checked files lack the header" >&2
  exit 1
fi
echo "license-check: $checked files carry '$want'"
