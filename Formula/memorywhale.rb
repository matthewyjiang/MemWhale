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
  url "https://github.com/wuisabel-gif/MemWhale/archive/refs/tags/v0.6.1.tar.gz"
  sha256 "0d7ab035ca61f3daf2c61c35712f3cb98acc962e47575206fd5b5816ce918a02"
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
