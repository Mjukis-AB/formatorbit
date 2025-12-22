class Forb < Formula
  desc "CLI tool that shows all possible interpretations of any data input"
  homepage "https://github.com/mjukis-ab/formatorbit"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mjukis-ab/formatorbit/releases/download/v0.1.0/forb-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "12420082e70337153845a76e1f53d6e39c5e93bd31989477fa7a9f2f9bf12f76"
    else
      url "https://github.com/mjukis-ab/formatorbit/releases/download/v0.1.0/forb-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "9dac0710705fc05ee59f933f095a5d55f5c111e1a01baaa7f95bb5d86e8606be"
    end
  end

  on_linux do
    url "https://github.com/mjukis-ab/formatorbit/releases/download/v0.1.0/forb-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "8e052b973d38405d4aa5c947cbcc43e6519320f78ffe809c57bb35111124ba5a"
  end

  def install
    bin.install "forb"
  end

  test do
    assert_match "hex", shell_output("#{bin}/forb --formats")
  end
end
