# OmniEdge Release Strategy

This document outlines the strategy for managing stable and test (beta/pre-release) versions of OmniEdge.

## 1. Versioning Scheme

We will follow [Semantic Versioning (SemVer)](https://semver.org/) with suffixes for pre-releases.

| Type | Format | Example | Description |
|------|--------|---------|-------------|
| **Stable** | `vX.Y.Z` | `v1.0.2` | Production-ready, officially supported. |
| **Beta** | `vX.Y.Z-beta.N` | `v1.0.2-beta.1` | Feature-complete, ready for community testing. |
| **RC** | `vX.Y.Z-rc.N` | `v1.0.2-rc.1` | Release candidate, fixing bugs from beta. |

## 2. Branching Model

| Branch | Purpose | Stability |
|--------|---------|-----------|
| `main` | Production code. Merges from `release/*` or `hotfix/*`. | **High** |
| `develop` | Integration branch for new features. | **Medium** |
| `feature/*` | Individual feature development. Merges into `develop`. | **Low** |
| `release/*` | Staging for next stable release. Branches from `develop`. | **High** |
| `hotfix/*` | Critical fixes for production. Branches from `main`. | **High** |

## 3. CI/CD Integration (GitHub Actions)

### Triggers
Documentation update for `.github/workflows/release.yml` and `desktop-release.yml`:

```yaml
on:
  push:
    tags:
      - 'v*.*.*'         # Stable Releases
      - 'v*.*.*-beta.*'  # Beta Releases
      - 'v*.*.*-rc.*'    # Release Candidates
```

### GitHub Release Marking
The `svenstaro/upload-release-action` should be configured to mark non-standard tags as "Pre-release":

```yaml
- name: Upload to Release
  uses: svenstaro/upload-release-action@v2
  with:
    repo_token: ${{ secrets.GITHUB_TOKEN }}
    file: ...
    tag: ${{ github.ref }}
    overwrite: true
    prerelease: ${{ contains(github.ref, '-beta') || contains(github.ref, '-rc') }}
```

## 4. Release Channels

| **Stable** | `vX.Y.Z` | Official Release | `latest` on website/docs |
| **Beta/RC** | `vX.Y.Z-suffix` | Pre-release | "Beta" downloads section |

