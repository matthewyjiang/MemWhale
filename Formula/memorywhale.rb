# Homebrew formula for the MemoryWhale CLI.
#
# Use as a tap:
#   brew tap wuisabel-gif/memorywhale https://github.com/wuisabel-gif/MemWhale
#   brew install memorywhale
#
# Maintainer note: after tagging a release, set `version`/`url` to the tag and
# fill `sha256` with:  curl -fsSL <url> | shasum -a 256
class Memorywhale < Formula
  desc "Local-first terminal memory: record commands, sessions, and output into SQLite"
  homepage "https://github.com/wuisabel-gif/MemWhale"
  url "https://github.com/wuisabel-gif/MemWhale/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "3dd726cd9d4ff5e7434e3cb94720415d0d2c54f0ca4024970a6a2d5df75e3966"
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
