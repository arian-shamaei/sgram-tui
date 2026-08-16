class SgramTui < Formula
  desc "Calibrated terminal spectrogram analyzer (live mic or audio files)"
  homepage "https://github.com/arian-shamaei/sgram-tui"
  url "https://github.com/arian-shamaei/sgram-tui/archive/refs/tags/v0.4.0.tar.gz"
  sha256 "6c7c8df6c750ab6fd411bf8920df366a9c4301b41fa9a488b3eae702d772b5be"
  license "MIT"

  head "https://github.com/arian-shamaei/sgram-tui.git", branch: "main"

  depends_on "rust" => :build

  on_linux do
    # cpal backend uses ALSA by default on Linux for microphone support
    depends_on "alsa-lib"
  end

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    help = shell_output("#{bin}/sgram-tui --help")
    assert_match "Terminal spectrogram viewer", help
  end
end
