# typed: false
# frozen_string_literal: true

class OmniedgeCli < Formula
  desc "Peer-to-peer VPN CLI for edge computing"
  homepage "https://omniedge.io"
  version "2.0.0"
  license "GPL-3.0"

  on_macos do
    on_arm do
      url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-cli-#{version}-macos-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_MACOS_ARM64"
    end
    on_intel do
      url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-cli-#{version}-macos-x64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_MACOS_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-cli-#{version}-linux-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_ARM64"
    end
    on_intel do
      url "https://github.com/omniedgeio/omniedge/releases/download/v#{version}/omniedge-cli-#{version}-linux-x64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X64"
    end
  end

  def install
    # Find the binary (name includes version)
    binary = Dir["omniedge-cli-*"].find { |f| File.executable?(f) && !f.end_with?(".tar.gz") }
    if binary
      bin.install binary => "omniedge"
    else
      # Fallback if binary has simple name
      bin.install "omniedge" if File.exist?("omniedge")
    end
  end

  def caveats
    <<~EOS
      To start using OmniEdge CLI:
        omniedge login
        omniedge join

      For more information, visit: https://omniedge.io/docs
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/omniedge --version")
  end
end
