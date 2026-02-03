# typed: false
# frozen_string_literal: true

cask "omniedge-desktop" do
  version "2.0.0"
  sha256 "PLACEHOLDER_SHA256"

  url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-desktop-#{version}-macos-#{Hardware::CPU.arm? ? "arm64" : "x64"}.dmg"
  name "OmniEdge Desktop"
  desc "Peer-to-peer VPN desktop client for edge computing"
  homepage "https://omniedge.io"

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
