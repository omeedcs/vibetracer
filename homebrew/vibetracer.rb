class Vibetracer < Formula
  desc "Real-time tracing, replaying, and rewinding of AI coding assistant edits"
  homepage "https://github.com/omeedcs/vibetracer"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/omeedcs/vibetracer/releases/download/v#{version}/vibetracer-aarch64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
    on_intel do
      url "https://github.com/omeedcs/vibetracer/releases/download/v#{version}/vibetracer-x86_64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/omeedcs/vibetracer/releases/download/v#{version}/vibetracer-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
    on_intel do
      url "https://github.com/omeedcs/vibetracer/releases/download/v#{version}/vibetracer-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  def install
    bin.install "vibetracer"
  end

  test do
    assert_match "vibetracer", shell_output("#{bin}/vibetracer --version")
  end
end
