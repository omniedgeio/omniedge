# typed: false
# frozen_string_literal: true

cask "omniedge-desktop" do
  version "2.3.0"
  sha256 arm: "8628e5fa065bc6afc586592be1f6962877afec793c0189667f106ff2a0f67d75",
         intel: "fc3853570c4d2cfe3d5faa1aa197de1051d7aebd48607f0a80371e10ee43ba54"

  url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-desktop-#{version}-macos-#{Hardware::CPU.arm? ? "arm64" : "x64"}.dmg"
  name "OmniEdge Desktop"
  desc "Zero-Config P2P Mesh VPN for AI, Robotics, and Edge Computing"
  homepage "https://connect.omniedge.io"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :monterey"

  app "OmniEdge.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/OmniEdge.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/io.omniedge.desktop",
    "~/Library/Caches/io.omniedge.desktop",
    "~/Library/Preferences/io.omniedge.desktop.plist",
    "~/Library/Saved Application State/io.omniedge.desktop.savedState",
  ]

  caveats <<~EOS
    OmniEdge Desktop requires administrator privileges to create VPN tunnels.
    You may be prompted for your password when connecting.
  EOS
end
