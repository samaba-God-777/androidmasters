#!/bin/zsh
# Builds the AndroidMasters installers from a Mac:
#   macOS  universal (Intel + Apple Silicon) .dmg, ad-hoc signed
#   Windows 10/11 x64 NSIS setup .exe, cross-compiled with cargo-xwin
# and copies them to ~/Downloads. Usage: tools/build_installers.sh [mac|windows|all]
# Requirements: Xcode 26+ (icon), rustup targets x86_64-apple-darwin, aarch64-apple-darwin,
# x86_64-pc-windows-msvc, cargo-xwin, Homebrew llvm and nsis.
set -euo pipefail
here=${0:A:h}
repo=${here:h}
what=${1:-all}
cd "$repo"
version=$(node -p "require('./src-tauri/tauri.conf.json').version")
out="$HOME/Downloads"

"$here/make_icon.sh"

if [[ $what == mac || $what == all ]]; then
  npx tauri build --target universal-apple-darwin --bundles dmg
  dmg=(src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg)
  cp "${dmg[1]}" "$out/AndroidMasters-$version-macOS-universal.dmg"
  echo "macOS: $out/AndroidMasters-$version-macOS-universal.dmg"
fi

if [[ $what == windows || $what == all ]]; then
  export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
  # makensis aborts with std::bad_alloc under the C locale.
  export LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8
  npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
  exe=(src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe)
  cp "${exe[1]}" "$out/AndroidMasters-$version-Windows-x64-setup.exe"
  echo "Windows: $out/AndroidMasters-$version-Windows-x64-setup.exe"
fi
