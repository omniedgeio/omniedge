# OmniEdge Package Publishing

This directory contains packaging files for various package managers.

## Supported Package Managers

| Platform | Package Manager | Package Type | Status |
|----------|-----------------|--------------|--------|
| macOS/Linux | Homebrew | Formula (CLI) / Cask (Desktop) | Template ready |
| Arch Linux | AUR | PKGBUILD | Template ready |
| Windows | Winget | Manifest | Template ready |
| Windows | Chocolatey | nuspec + PowerShell | Template ready |
| Linux | Snap | snapcraft.yaml | Template ready |

## Directory Structure

```
packaging/
├── homebrew/
│   ├── Formula/
│   │   └── omniedge-cli.rb      # Homebrew formula for CLI
│   └── Casks/
│       └── omniedge-desktop.rb  # Homebrew cask for Desktop
├── aur/
│   ├── omniedge-cli/
│   │   └── PKGBUILD             # AUR package for CLI
│   └── omniedge-desktop/
│       └── PKGBUILD             # AUR package for Desktop

├── winget/
│   ├── omniedge-cli/
│   │   ├── OmniEdge.OmniEdgeCLI.yaml
│   │   ├── OmniEdge.OmniEdgeCLI.installer.yaml
│   │   └── OmniEdge.OmniEdgeCLI.locale.en-US.yaml
│   └── omniedge-desktop/
│       ├── OmniEdge.OmniEdgeDesktop.yaml
│       ├── OmniEdge.OmniEdgeDesktop.installer.yaml
│       └── OmniEdge.OmniEdgeDesktop.locale.en-US.yaml
├── chocolatey/
│   ├── omniedge-cli/
│   │   ├── omniedge-cli.nuspec
│   │   └── tools/
│   │       ├── chocolateyInstall.ps1
│   │       └── chocolateyUninstall.ps1
│   └── omniedge-desktop/
│       ├── omniedge-desktop.nuspec
│       └── tools/
│           ├── chocolateyInstall.ps1
│           └── chocolateyUninstall.ps1
└── snap/
    └── snapcraft.yaml           # Snap package for CLI
```

## Automatic Publishing

The `publish-packages.yml` workflow automatically updates and publishes packages when a new release is created.

### Required Secrets

| Secret | Description | Required For |
|--------|-------------|--------------|
| `HOMEBREW_TAP_TOKEN` | GitHub token with repo access | Homebrew Tap |
| `AUR_SSH_PRIVATE_KEY` | SSH key registered with AUR | AUR |
| `SNAPCRAFT_STORE_CREDENTIALS` | Snapcraft login credentials | Snap Store |
| `WINGET_TOKEN` | GitHub token for winget-pkgs PRs | Winget |
| `CHOCOLATEY_API_KEY` | Chocolatey.org API key | Chocolatey |

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `HOMEBREW_TAP_REPO` | Homebrew tap repository | `omniedge/homebrew-omniedge` |

## Manual Publishing

### Homebrew

1. Create a tap repository: `omniedge/homebrew-omniedge`
2. Copy formula/cask files to the tap
3. Update SHA256 checksums
4. Users install with:
   ```bash
   brew tap omniedge/omniedge
   brew install omniedge-cli
   brew install --cask omniedge-desktop
   ```

### AUR

1. Register at https://aur.archlinux.org
2. Add SSH key to profile
3. Clone and push:
   ```bash
   git clone ssh://aur@aur.archlinux.org/omniedge-cli.git
   cp packaging/aur/omniedge-cli/PKGBUILD omniedge-cli/
   cd omniedge-cli
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "Update to version X.Y.Z"
   git push
   ```
4. Repeat for Desktop:
   ```bash
   git clone ssh://aur@aur.archlinux.org/omniedge-desktop.git
   cp packaging/aur/omniedge-desktop/PKGBUILD omniedge-desktop/
   cd omniedge-desktop
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "Update to version X.Y.Z"
   git push
   ```



### Winget

1. Fork https://github.com/microsoft/winget-pkgs
2. Copy manifests to `manifests/o/OmniEdge/OmniEdgeCLI/<version>/`
3. Update version, installer URL, and checksums
4. Submit PR


### Chocolatey

1. Register at https://chocolatey.org
2. Get API key from account
3. Build and push CLI:
   ```powershell
   cd packaging/chocolatey/omniedge-cli
   choco pack
   choco push omniedge-cli.X.Y.Z.nupkg --source https://push.chocolatey.org/ --api-key YOUR_API_KEY
   ```
4. Build and push Desktop:
   ```powershell
   cd packaging/chocolatey/omniedge-desktop
   choco pack
   choco push omniedge-desktop.X.Y.Z.nupkg --source https://push.chocolatey.org/ --api-key YOUR_API_KEY
   ```


### Snap

1. Register at https://snapcraft.io
2. Register snap name: `snapcraft register omniedge-cli`
3. Build and publish:
   ```bash
   cd packaging/snap
   snapcraft
   snapcraft upload --release=stable omniedge-cli_*.snap
   ```

## Updating Packages

When releasing a new version:

1. Update version numbers in all package files
2. Update SHA256 checksums (computed from release assets)
3. Test packages locally if possible
4. Push updates to respective repositories

The `publish-packages.yml` workflow automates this process.
