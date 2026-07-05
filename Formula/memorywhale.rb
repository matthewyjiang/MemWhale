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
  url "https://github.com/wuisabel-gif/MemWhale/archive/refs/tags/v0.3.1.tar.gz"
  sha256 "1da26472e9a326745419b0abed159d2aa9d62ef1df600d9146a9843bb34996ee"
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
