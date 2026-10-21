#!/usr/bin/env bash
# Kiểm tra môi trường build. In ra nền tảng nào build được trên máy này.
# Dùng: bash scripts/check-env.sh        (in báo cáo)
#       source scripts/check-env.sh       (build-all.sh dùng các biến CAN_*)
set -u

ok()   { printf '  \033[32m✔\033[0m %s\n' "$*"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$*"; }
bad()  { printf '  \033[31m✘\033[0m %s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

case "$(uname -s)" in
  Linux*)               HOST_OS=linux ;;
  Darwin*)              HOST_OS=macos ;;
  MINGW*|MSYS*|CYGWIN*) HOST_OS=windows ;;
  *)                    HOST_OS=unknown ;;
esac

CAN_LINUX=0; CAN_WINDOWS=0; CAN_IOS=0; CAN_MACOS=0
ENV_ERRORS=()

echo "Hệ điều hành: $HOST_OS ($(uname -m))"
echo "Công cụ chung:"

if have node; then
  NODE_MAJOR=$(node -p 'process.versions.node.split(".")[0]')
  if [ "$NODE_MAJOR" -ge 18 ]; then ok "Node.js $(node -v)"; else bad "Node.js $(node -v) quá cũ (cần >= 18)"; ENV_ERRORS+=("node"); fi
else
  bad "Thiếu Node.js >= 18 → https://nodejs.org"; ENV_ERRORS+=("node")
fi
have npm && ok "npm $(npm -v)" || { bad "Thiếu npm"; ENV_ERRORS+=("npm"); }

if have cargo && have rustc; then
  ok "Rust $(rustc --version | awk '{print $2}')"
else
  bad "Thiếu Rust → https://rustup.rs (Windows: cài thêm 'Desktop development with C++' của Visual Studio Build Tools)"
  ENV_ERRORS+=("rust")
fi

if [ -d node_modules/@tauri-apps/cli ]; then ok "Tauri CLI đã cài (node_modules)"; else warn "Chưa cài Tauri CLI → chạy: npm install"; fi

COMMON_OK=1; [ ${#ENV_ERRORS[@]} -eq 0 ] || COMMON_OK=0

echo "Theo nền tảng:"
case "$HOST_OS" in
  linux)
    MISSING=()
    for pc in webkit2gtk-4.1 gtk+-3.0 librsvg-2.0; do
      pkg-config --exists "$pc" 2>/dev/null || MISSING+=("$pc")
    done
    if [ ${#MISSING[@]} -eq 0 ]; then
      ok "WebKitGTK 4.1 $(pkg-config --modversion webkit2gtk-4.1), GTK3, librsvg"
      [ $COMMON_OK -eq 1 ] && CAN_LINUX=1
    else
      bad "Thiếu thư viện: ${MISSING[*]}"
      echo "      Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev build-essential file patchelf"
      echo "      Fedora:        sudo dnf install webkit2gtk4.1-devel gtk3-devel librsvg2-devel libappindicator-gtk3-devel openssl-devel"
    fi
    have rpmbuild || warn "Không có rpmbuild: gói .rpm vẫn được Tauri tự tạo (không cần rpmbuild)."
    bad "Windows: không build được từ Linux (cần máy Windows hoặc GitHub Actions)"
    bad "iOS: bắt buộc macOS + Xcode (Apple không cho build iOS trên Linux)"
    ;;
  windows)
    if [ $COMMON_OK -eq 1 ]; then
      if have cl.exe || [ -d "/c/Program Files (x86)/Microsoft Visual Studio" ] || [ -d "/c/Program Files/Microsoft Visual Studio" ]; then
        ok "Tìm thấy Visual Studio / Build Tools"; CAN_WINDOWS=1
      else
        warn "Không chắc đã có MSVC Build Tools (C++). Vẫn thử build; nếu lỗi linker → cài 'Desktop development with C++'."
        CAN_WINDOWS=1
      fi
    fi
    ok "WebView2: có sẵn trên Windows 10/11 cập nhật; bộ cài tự tải nếu thiếu"
    bad "Linux: không build được từ Windows (dùng máy Linux, WSL2 hoặc GitHub Actions)"
    bad "iOS: bắt buộc macOS + Xcode"
    ;;
  macos)
    if have xcodebuild && xcodebuild -version >/dev/null 2>&1; then
      ok "$(xcodebuild -version | head -1)"
      [ $COMMON_OK -eq 1 ] && CAN_MACOS=1
      if have rustup && rustup target list --installed | grep -q aarch64-apple-ios; then
        ok "Rust target aarch64-apple-ios"
      else
        warn "Thiếu Rust target iOS → rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios"
      fi
      have pod && ok "CocoaPods $(pod --version)" || warn "Không có CocoaPods (thường không bắt buộc với Tauri v2)"
      [ $COMMON_OK -eq 1 ] && CAN_IOS=1
      if [ -z "${APPLE_DEVELOPMENT_TEAM:-}" ] && [ ! -d src-tauri/gen/apple ]; then
        warn "Chưa đặt APPLE_DEVELOPMENT_TEAM (Team ID 10 ký tự) → cần để ký app iOS"
      fi
    else
      bad "Thiếu Xcode đầy đủ (không phải chỉ Command Line Tools) → cài từ App Store, rồi: sudo xcode-select -s /Applications/Xcode.app"
    fi
    bad "Windows: không build được từ macOS (dùng GitHub Actions)"
    bad "Linux: không build được từ macOS (dùng GitHub Actions)"
    ;;
  *)
    bad "Hệ điều hành không hỗ trợ: $(uname -s)"
    ;;
esac

echo "Có thể build trên máy này: linux=$CAN_LINUX windows=$CAN_WINDOWS ios=$CAN_IOS macos=$CAN_MACOS"
export HOST_OS CAN_LINUX CAN_WINDOWS CAN_IOS CAN_MACOS
