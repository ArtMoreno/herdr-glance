# Local source installation. Create the archive with `cargo package` first.
require "digest"
require "uri"

class HerdrGlance < Formula
  desc "Themeable Herdr agent status dashboard"
  homepage "https://github.com/ArtMoreno/herdr-glance"
  version "0.1.0"
  license "MIT"
  archive = Pathname.new(ENV.fetch("HERDR_GLANCE_SOURCE"))/"target/package/herdr-glance-0.1.0.crate"
  url "file://#{URI::DEFAULT_PARSER.escape(archive.realpath.to_s)}"
  sha256 Digest::SHA256.file(archive).hexdigest
  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--path", ".", "--root", prefix
  end

  test do
    assert_match "herdr-glance", shell_output("#{bin}/glance --help")
  end
end
