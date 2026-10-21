#!/usr/bin/env bash
# Build mọi nền tảng mà máy hiện tại hỗ trợ, gom artifact vào dist/<nền tảng>/.
#   ./build-all.sh              build tất cả nền tảng khả dụng
#   ./build-all.sh --ios-sim    (macOS) build thêm bản iOS Simulator không cần ký
#   ./build-all.sh --no-ios     bỏ qua iOS
# Biến môi trường iOS: APPLE_DEVELOPMENT_TEAM, IOS_EXPORT_METHOD (debugging|release-testing|app-store-connect)
# Ký thủ công trên CI: IOS_CERTIFICATE, IOS_CERTIFICATE_PASSWORD, IOS_MOBILE_PROVISION (base64)
set -uo pipefail
cd "$(dirname "$0")"

IOS_SIM=0; SKIP_IOS=0
for a in "$@"; do
  case "$a" in
    --ios-sim) IOS_SIM=1 ;;
    --no-ios)  SKIP_IOS=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "Tham số không hợp lệ: $a"; exit 2 ;;
  esac
done

echo "== Kiểm tra môi trường =="
# shellcheck source=scripts/check-env.sh
source scripts/check-env.sh
echo

VERSION=$(node -p 'require("./package.json").version' 2>/dev/null || echo "0.0.0")
OUT=dist
BUNDLE=src-tauri/target/release/bundle
declare -A RESULT

if [ ! -d node_modules/@tauri-apps/cli ]; then
  echo "== npm install =="
  npm install || { echo "npm install thất bại"; exit 1; }
fi

# Sao chép artifact thật; trả về lỗi nếu không có file nào khớp.
collect() { # $1=đích  $2..=glob
  local dest="$1"; shift; local n=0
  mkdir -p "$dest"
  shopt -s nullglob
  for f in "$@"; do
    if [ -e "$f" ]; then cp -R "$f" "$dest"/ && n=$((n+1)); fi
  done
  shopt -u nullglob
  [ $n -gt 0 ]
}

run_step() { # $1=tên  $2..=lệnh
  local name="$1"; shift
  echo "== Build $name =="
  if "$@"; then return 0; else echo "!! Lệnh build $name trả về lỗi"; return 1; fi
}

# ---------- Desktop của máy hiện tại ----------
if [ "$CAN_LINUX" = 1 ]; then
  rm -rf "$OUT/linux"
  if run_step linux npx tauri build --bundles appimage,deb,rpm \
     && collect "$OUT/linux" "$BUNDLE"/appimage/*.AppImage "$BUNDLE"/deb/*.deb "$BUNDLE"/rpm/*.rpm; then
    RESULT[linux]="OK"
  else RESULT[linux]="THẤT BẠI"; fi
elif [ "$HOST_OS" != linux ]; then RESULT[linux]="BỎ QUA (cần máy Linux hoặc CI)"
else RESULT[linux]="BỎ QUA (thiếu dependencies, xem trên)"; fi

if [ "$CAN_WINDOWS" = 1 ]; then
  rm -rf "$OUT/windows"
  if run_step windows npx tauri build --bundles nsis,msi \
     && collect "$OUT/windows" "$BUNDLE"/nsis/*.exe "$BUNDLE"/msi/*.msi; then
    # Bản portable: exe gốc (cần WebView2 có sẵn trên máy đích).
    cp src-tauri/target/release/nebula-wrapper.exe "$OUT/windows/ZyxelNebula-${VERSION}-portable.exe" 2>/dev/null || true
    RESULT[windows]="OK"
  else RESULT[windows]="THẤT BẠI"; fi
elif [ "$HOST_OS" != windows ]; then RESULT[windows]="BỎ QUA (cần Windows hoặc CI)"
else RESULT[windows]="BỎ QUA (thiếu công cụ, xem trên)"; fi

if [ "$CAN_MACOS" = 1 ]; then
  rm -rf "$OUT/macos"
  if run_step macos npx tauri build --bundles app,dmg \
     && collect "$OUT/macos" "$BUNDLE"/dmg/*.dmg; then RESULT[macos]="OK (bổ sung)"
  else RESULT[macos]="THẤT BẠI"; fi
fi

# ---------- iOS ----------
if [ "$SKIP_IOS" = 1 ]; then RESULT[ios]="BỎ QUA (--no-ios)"
elif [ "$CAN_IOS" = 1 ]; then
  rm -rf "$OUT/ios"; mkdir -p "$OUT/ios"
  IOS_OK=1
  if [ ! -d src-tauri/gen/apple ]; then
    if [ -z "${APPLE_DEVELOPMENT_TEAM:-}" ]; then
      echo "!! Thiếu APPLE_DEVELOPMENT_TEAM: không thể tạo dự án Xcode có cấu hình ký."
      IOS_OK=0; RESULT[ios]="BỎ QUA (thiếu APPLE_DEVELOPMENT_TEAM)"
    else
      run_step "ios init" npx tauri ios init --ci || IOS_OK=0
    fi
  fi
  if [ $IOS_OK = 1 ]; then
    if [ $IOS_SIM = 1 ]; then
      if run_step "ios simulator" npx tauri ios build --debug --target aarch64-sim; then
        mkdir -p "$OUT/ios/simulator"
        find src-tauri/gen/apple/build -maxdepth 5 -type d -name '*.app' -exec cp -R {} "$OUT/ios/simulator/" \; 2>/dev/null || true
      fi
    fi
    METHOD="${IOS_EXPORT_METHOD:-release-testing}"
    if run_step "ios ($METHOD)" npx tauri ios build --export-method "$METHOD" \
       && collect "$OUT/ios" src-tauri/gen/apple/build/arm64/*.ipa; then
      RESULT[ios]="OK ($METHOD)"
    else
      RESULT[ios]="THẤT BẠI (thường do ký ứng dụng: Team ID, chứng chỉ, provisioning profile)"
    fi
  fi
elif [ "$HOST_OS" != macos ]; then RESULT[ios]="BỎ QUA (iOS chỉ build được trên macOS + Xcode)"
else RESULT[ios]="BỎ QUA (thiếu Xcode)"; fi

# ---------- Báo cáo ----------
echo
echo "================ KẾT QUẢ (v$VERSION) ================"
FAIL=0
for p in linux windows ios macos; do
  [ -n "${RESULT[$p]:-}" ] || continue
  printf '%-8s %s\n' "$p" "${RESULT[$p]}"
  [[ "${RESULT[$p]}" == THẤT* ]] && FAIL=1
done
echo "-----------------------------------------------------"
if [ -d "$OUT" ]; then
  find "$OUT" -maxdepth 2 -type f \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' -o -name '*.exe' -o -name '*.msi' -o -name '*.ipa' -o -name '*.dmg' \) \
    -exec sh -c 'for f; do printf "%8s  %s\n" "$(du -h "$f" | cut -f1)" "$f"; done' _ {} +
  if command -v sha256sum >/dev/null; then
    (cd "$OUT" && find . -type f ! -name SHA256SUMS -exec sha256sum {} + > SHA256SUMS 2>/dev/null)
  elif command -v shasum >/dev/null; then
    (cd "$OUT" && find . -type f ! -name SHA256SUMS -exec shasum -a 256 {} + > SHA256SUMS 2>/dev/null)
  fi
fi
exit $FAIL
