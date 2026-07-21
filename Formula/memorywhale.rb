# Homebrew formula for the MemoryWhale CLI.
#
# Use as a tap:
#   brew tap wuisabel-gif/memorywhale https://github.com/wuisabel-gif/MemWhale
#   brew install memorywhale
#
# Maintainer note: `url`/`sha256` are updated automatically by the
# bump-formula job in .github/workflows/release.yml on every tagged release.
class Memorywhale < Formula
  desc "Local-first terminal memory: record commands, sessions, and output into SQLite"
  homepage "https://github.com/wuisabel-gif/MemWhale"
  url "https://github.com/wuisabel-gif/MemWhale/archive/refs/tags/v0.6.0.tar.gz"
  sha256 "9d0d40b938c9c283afd0ee9a15795e53ac9727a7c5de204d7df23db3b9285da3"
  license "MIT"
  head "https://github.com/wuisabel-gif/MemWhale.git", branch: "main"

  depends_on "rust" => :build

  def install
    # Build only the dependency-light CLI crate (no Tauri/GTK).
    system "cargo", "install", "--path", "crates/mw-cli", "--root", prefix
  end

  test do
    assert_match "record a whole shell session", shell_output("#{bin}/mw --help")
  end
end
