# Lint a policy repository

`notaio lint` runs the controller's own checks on policy files, so a change that would not publish
fails its pull request instead ([ADR-0006](../adr/0006-notaio-lint-cli.md)). Same rules, same
codes, same ceilings, no cluster needed.

## Running it

```sh
notaio lint policies/                                  # text; exit 1 on any error finding
notaio lint --format sarif policies/ > notaio.sarif    # for inline review
notaio lint --now 2026-10-20T00:00:00Z policies/       # pin the clock for a reproducible run
```

It reads every `.yaml` and `.yml` file under the paths given, picks out the four kinds, and
ignores other documents. Policies may reference the built-in profiles without the files being
present.

| Exit code | Meaning |
| --- | --- |
| 0 | No error findings. Warnings are allowed. |
| 1 | At least one error finding. |
| 2 | An input could not be read or parsed. |

Findings carry the policy codes `L001` to `L016` and the input codes `I001` to `I008`, about the
set of files given (a duplicate object, a policy whose profile is missing, and so on). All of them
are in [Lint Rules](../reference/lint-rules.md).

Grant expiry and lifetime checks depend on the clock. CI uses the real time, so a grant that is
fine today can fail a later, unrelated pull request once it has expired: that is the point. Use
`--now` only for reproducing a past run.

## On every pull request, with GitHub Actions

Run the published image, pinned by digest, and upload the SARIF so findings appear inline:

```yaml
name: Policy lint
on:
  pull_request:
permissions:
  contents: read
jobs:
  lint:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: notaio lint
        run: |
          docker run --rm -v "$PWD:/repo:ro" -w /repo \
            ghcr.io/firestoned/notaio@sha256:<digest> \
            lint --format sarif . > notaio.sarif
      - name: Upload findings
        if: always()
        uses: github/codeql-action/upload-sarif@1c5b675653bb5c22dbe9b12b556ec555138e09fd # v4.38.1
        with:
          sarif_file: notaio.sarif
          category: notaio
```

The upload runs even when the lint fails, so the findings appear on the pull request while the
lint step keeps it red. Take the digest from a release's `notaio-<version>.yaml`, and make the job
a required check in the repository's branch protection.

## Laying out the repository

A layout that keeps the second approval for ceilings real:

```text
policies/
  team-docs/
    sandboxpolicy-docs-agent.yaml
    knobgrant-docs-agent-pkg-mirror.yaml
ceilings/                 # CODEOWNERS: @org/security
  team-docs/
    exposureceiling-docs-agent.yaml
```

Lint the whole repository (`.`), not one directory, so each policy is checked with its ceiling
and grants.
