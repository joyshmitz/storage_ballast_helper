class Sbh < Formula
  desc "Disk-pressure defense system for AI coding workloads"
  homepage "https://github.com/Dicklesworthstone/storage_ballast_helper"
  version "0.4.7"
  license "MIT"

  # Release automation in Dicklesworthstone/homebrew-sbh must replace both
  # placeholder checksums when copying this skeleton into the tap.
  on_macos do
    on_arm do
      url "https://github.com/Dicklesworthstone/storage_ballast_helper/releases/download/v#{version}/" \
          "sbh-v#{version}-aarch64-apple-darwin.tar.xz"
      sha256 "REPLACE_WITH_AARCH64_APPLE_DARWIN_SHA256"
    end

    on_intel do
      url "https://github.com/Dicklesworthstone/storage_ballast_helper/releases/download/v#{version}/" \
          "sbh-v#{version}-x86_64-apple-darwin.tar.xz"
      sha256 "REPLACE_WITH_X86_64_APPLE_DARWIN_SHA256"
    end
  end

  def install
    bin.install "sbh"
  end

  def post_install
    system bin/"sbh", "setup", "--verify", "--bin-dir", bin
  end

  service do
    run [opt_bin/"sbh", "daemon"]
    keep_alive crashed: true
    process_type :background
    throttle_interval 60
    environment_variables PATH: std_service_path_env
    log_path var/"log/sbh.log"
    error_log_path var/"log/sbh.err.log"
  end

  def caveats
    <<~EOS
      Finish interactive setup when you want shell PATH/completion changes:
        sbh setup --all --bin-dir #{HOMEBREW_PREFIX}/bin

      Start the daemon with Homebrew services:
        brew services start sbh

      On macOS, grant Full Disk Access to the installed sbh binary if scans need
      to inspect protected user locations:
        #{HOMEBREW_PREFIX}/bin/sbh doctor --pal
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sbh --version")
  end
end
