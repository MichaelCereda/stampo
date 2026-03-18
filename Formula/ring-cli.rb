class RingCli < Formula
  desc "Generate CLIs from YAML configs and OpenAPI specs"
  homepage "https://github.com/MichaelCereda/ring-cli"
  version "2.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Darwin-aarch64.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Darwin-x86_64.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Linux-aarch64-musl.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Linux-x86_64-musl.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "ring-cli"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ring-cli --version")
  end
end
