# Zyxel Nebula Wrapper (Tauri v2)

Ứng dụng "trình duyệt chuyên dụng" chỉ phục vụ `https://nebula.zyxel.com`, dùng **một bộ mã nguồn** cho iOS, Windows và Linux (kèm macOS nếu cần).

---

## 1. Kết luận

**Chọn Tauri v2.** Đây là phương án duy nhất trong nhóm được đánh giá vừa có iOS, Windows và Linux từ cùng một dự án, vừa dùng WebView có sẵn của hệ điều hành (gói nhỏ), vừa cho phép chặn điều hướng ở tầng native (allowlist domain thật sự, không phải chỉ là lời khuyên trong JS).

| Tiêu chí | PWA | **Tauri v2** | Electron | Flutter + WebView |
|---|---|---|---|---|
| iOS | "Thêm vào MH chính" từ Safari, không cần ký | Có (WKWebView), cần macOS + Xcode + ký | **Không hỗ trợ** | Có (webview_flutter) |
| Windows | Cài từ Edge/Chrome | NSIS `.exe` / `.msi` (WebView2) | Có | Chỉ qua plugin cộng đồng (`webview_windows`), API khác iOS |
| Linux | Cài từ Chrome/Chromium | AppImage / `.deb` / `.rpm` (WebKitGTK) | Có | Không có plugin WebView chính thức, ổn định |
| Dùng chung mã | 100% (chính là website) | ~100% (1 file Rust + 1 file cấu hình) | Desktop only → vẫn cần dự án iOS riêng | Phải viết riêng cho từng plugin WebView |
| Dung lượng | 0 | `.deb` 1,9 MB, AppImage ~78 MB (đã đo); Windows: vài MB (ước tính) | ~100 MB+ mỗi bản | ~20–40 MB |
| Đăng nhập/session | Tốt nhất (trình duyệt thật) | Cookie lưu bền; giới hạn với Google/Passkey (mục 9) | Tốt | Tương tự Tauri |
| Giới hạn domain | **Không làm được** | Native `on_navigation` + `on_new_window` | Làm được | Làm được trên iOS; desktop phụ thuộc plugin |
| Tự động hóa | Không cần build | `./build-all.sh` + GitHub Actions | Tốt | Trung bình |
| Bảo trì | Không | Thấp (1 dependency chính) | Cập nhật Chromium liên tục | Trung bình–cao |
| Chi phí | 0 | Apple Developer Program 99 USD/năm cho iOS; ký Windows tùy chọn | Như Tauri (không có iOS) | Như Tauri |

**Phương án dự phòng không cần build:** nếu chỉ cần "biểu tượng Nebula trên màn hình", PWA (Safari → *Chia sẻ → Thêm vào MH chính*; Edge → *Ứng dụng → Cài đặt trang web này*) là đơn giản nhất. Nhưng nó không thực hiện được yêu cầu chặn domain và chính sách liên kết ngoài, nên không được chọn làm phương án chính.

## 2. Giới hạn: "một" đến mức nào

| | Được | Không được (và lý do) |
|---|---|---|
| Mã nguồn | **1 bộ** cho mọi nền tảng | — |
| Cấu hình domain | **1 file** `nebula.config.json` | — |
| Lệnh build | **1 lệnh** `./build-all.sh` trên mỗi máy; **1 lần bấm** trên GitHub Actions cho cả 3 nền tảng | Một máy không build được hết: iOS bắt buộc macOS + Xcode (điều khoản và toolchain của Apple); Tauri không hỗ trợ cross-compile Windows ⇄ Linux ổn định |
| File cài đặt | **Mỗi nền tảng 1 file** | **Không tồn tại một file cài chạy được trên cả iOS, Windows và Linux.** Ba hệ điều hành dùng định dạng thực thi khác nhau (Mach-O ARM64 có chữ ký Apple, PE/COFF, ELF), cơ chế cài khác nhau (iOS chỉ cài app đã ký qua kênh Apple cho phép) và WebView khác nhau (WKWebView, WebView2, WebKitGTK) |

Cách gần nhất với "một file": một **GitHub Release** (hoặc thư mục chia sẻ) chứa `.ipa`, `.exe`/`.msi`, `.AppImage`/`.deb`/`.rpm`, kèm `SHA256SUMS`, tạo tự động khi push tag.

## 3. Sơ đồ kiến trúc

```
                    nebula.config.json  (startUrl, mainHosts, authHosts, externalLinks)
                              │ nhúng lúc build
┌─────────────────────────────▼──────────────────────────────────────────────┐
│ Tiến trình Rust (src-tauri/src/lib.rs)                                     │
│  • on_navigation  ──► navigation_decision(url)                             │
│        Allow  : origin cục bộ | about:blank | blob: |                      │
│                 https + host ∈ mainHosts ∪ authHosts |                     │
│                 tiền tố iframe đã kiểm chứng (reCAPTCHA, YouTube embed)    │
│        Mở ngoài: nebula-external:<url> (người dùng bấm link ngoài)         │
│        Chặn  : mọi thứ khác (http://, data:, file:, domain lạ) + ghi log   │
│  • on_new_window (desktop): popup trong allowlist → cửa sổ con có opener;  │
│                             ngoài allowlist → trình duyệt hệ thống         │
│  • Menu desktop: Quay lại / Tiến tới / Tải lại / Trang chủ                 │
│  • Capabilities: RỖNG → trang web không gọi được lệnh native nào           │
└───────────────┬────────────────────────────────────────────────────────────┘
                │ WebView hệ thống: WKWebView (iOS) · WebView2 (Windows) · WebKitGTK (Linux)
┌───────────────▼───────────────┐    ┌──────────────────────────────────────┐
│ Trang khởi chạy cục bộ        │───►│ https://nebula.zyxel.com             │
│ src/index.html + app.js       │    │   ↕ đăng nhập                        │
│ kiểm tra mạng, timeout,       │    │ https://account.zyxel.com            │
│ nút Thử lại                   │    │   ↕ SSO: appleid.apple.com,          │
└───────────────────────────────┘    │     login.microsoftonline.com …      │
                                     │ + script nhỏ (main frame, không IPC):│
                                     │   link ngoài → trình duyệt hệ thống, │
                                     │   thanh báo mất mạng + Tải lại       │
                                     └──────────────────────────────────────┘
```

**Domain đã kiểm chứng ngày 20/10/2026** bằng cách mở Nebula trong trình duyệt thật và bấm *Get Started* (không đăng nhập):
- `nebula.zyxel.com` chuyển hướng tới `account.zyxel.com/users/sign_in`.
- Trang đăng nhập có: Zyxel Account (email/mật khẩu), *Continue with Google / Apple / Microsoft Entra ID / Passkey*, và iframe reCAPTCHA Enterprise từ `www.google.com/recaptcha/…`.
- Trang chủ nhúng video `www.youtube-nocookie.com/embed/…`.
- `appleid.apple.com`, `login.microsoftonline.com`, `login.live.com` là domain đăng nhập chính thức của Apple/Microsoft, **chưa được kiểm thử qua luồng bấm thật**. Xem mục 8 để xác nhận.

## 4. Cấu trúc thư mục

```
nebula-wrapper/
├── nebula.config.json        ← đổi domain/allowlist tại đây
├── package.json              ← version ứng dụng (dùng chung mọi nền tảng)
├── app-icon.png              ← icon gốc 1024×1024 (đang là icon tạm, xem mục 9)
├── build-all.sh              ← build mọi nền tảng khả dụng trên máy hiện tại
├── scripts/check-env.sh      ← kiểm tra công cụ, báo lỗi dễ hiểu
├── src/                      ← màn hình khởi chạy cục bộ (không dùng IPC)
│   ├── index.html  app.js  style.css  logo.png
├── src-tauri/
│   ├── Cargo.toml  build.rs  tauri.conf.json
│   ├── capabilities/default.json   ← cố ý không cấp quyền nào
│   ├── icons/                      ← sinh bằng `npm run icon`
│   ├── src/main.rs  src/lib.rs     ← toàn bộ logic (kèm unit test)
│   └── gen/apple/                  ← sinh bởi `tauri ios init` (chỉ trên macOS)
└── .github/workflows/release.yml   ← build Windows + Linux + iOS trên GitHub
```

## 5. Cài đặt môi trường

Chung: **Node.js ≥ 18**, **Rust** (https://rustup.rs), rồi trong thư mục dự án chạy `npm install`.

| Máy build | Cài thêm |
|---|---|
| Linux (Debian/Ubuntu) | `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev build-essential file patchelf` |
| Linux (Fedora) | `sudo dnf install webkit2gtk4.1-devel gtk3-devel librsvg2-devel libappindicator-gtk3-devel openssl-devel` |
| Windows 10/11 | Visual Studio Build Tools → "Desktop development with C++"; Git for Windows (để chạy `build-all.sh` bằng Git Bash). WebView2 có sẵn trên Windows 10/11 cập nhật |
| macOS (cho iOS) | Xcode đầy đủ từ App Store; `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`; tài khoản Apple Developer |

Kiểm tra: `npm run check-env`. Script sẽ in ra nền tảng nào build được trên máy này, thiếu gì, và lệnh cài cụ thể.

## 6. Phát triển, build và phát hành

```bash
npm install
npm run dev                 # chạy thử desktop (Linux/Windows/macOS), có log "[nebula] chặn điều hướng…"
cargo test --manifest-path src-tauri/Cargo.toml   # test chính sách điều hướng
./build-all.sh              # build mọi nền tảng khả dụng → dist/<nền tảng>/ + dist/SHA256SUMS
```

`build-all.sh` sẽ:
- phát hiện OS và công cụ;
- build nền tảng tương thích;
- bỏ qua nền tảng không hỗ trợ kèm lý do;
- chỉ báo **OK** khi file artifact thực sự tồn tại;
- trả mã thoát ≠ 0 nếu có nền tảng thất bại.

**Đổi phiên bản:** sửa `version` trong `package.json`. **Đổi icon:** thay `app-icon.png` → `npm run icon`. **Đổi domain:** sửa `nebula.config.json` và `connect-src` trong `tauri.conf.json`. **Đổi tên/bundle id:** `productName`, `identifier` trong `tauri.conf.json` (hiện là `vn.ai.zyxel.nebula`, đặt theo giả định, nên thay bằng ID chính thức của tổ chức trước khi đăng ký với Apple).

**Phát hành bằng CI (khuyến nghị):** đẩy mã lên GitHub → `git tag v1.0.0 && git push --tags` → workflow build 3 nền tảng song song và tạo **draft release** chứa mọi file cài + `SHA256SUMS`. Có thể chạy thủ công ở *Actions → build → Run workflow*.

## 7. Artifact theo nền tảng

### Linux (đã build và chạy thử thực tế)
- `Zyxel Nebula_<ver>_amd64.AppImage`: chạy được trên hầu hết bản phân phối x86_64 (`chmod +x` rồi chạy). Gói này đã kèm WebKitGTK nên nặng khoảng 78 MB.
- `.deb` (Ubuntu/Debian) và `.rpm` (Fedora/RHEL) khoảng 1,9 MB, dùng WebKitGTK 4.1 của hệ thống: `sudo apt install ./Zyxel*.deb`.
- Phụ thuộc runtime: `libwebkit2gtk-4.1-0`, `libgtk-3-0`. Hệ điều hành cần có WebKitGTK 4.1, tức Ubuntu 22.04+ / Debian 12+ / Fedora 36+. Danh sách này là suy luận từ phiên bản thư viện; mới kiểm thử thực tế trên Ubuntu 24.04. CI build trên Ubuntu 22.04 để tương thích glibc rộng hơn.

### Windows (chưa build trong phiên này: cần máy Windows hoặc CI)
- `Zyxel Nebula_<ver>_x64-setup.exe` (NSIS, cài cho người dùng hiện tại, không cần quyền admin, tạo shortcut Start Menu/Desktop) và `.msi` (triển khai qua GPO/Intune).
- Runtime WebView2: bộ cài tự tải nếu máy thiếu (`downloadBootstrapper`). Máy offline hoàn toàn: đổi sang `offlineInstaller` trong `tauri.conf.json` (bộ cài nặng thêm khoảng 130 MB).
- Bản portable: `dist/windows/ZyxelNebula-<ver>-portable.exe` (yêu cầu WebView2 đã có sẵn).
- Chưa ký số → SmartScreen sẽ cảnh báo. Có thể ký sau bằng chứng chỉ code signing (`bundle.windows.certificateThumbprint` hoặc Azure Trusted Signing).

### iOS (chỉ build được trên macOS)
1. `export APPLE_DEVELOPMENT_TEAM=XXXXXXXXXX` (Team ID) → `npm run ios:init` (tạo `src-tauri/gen/apple`, nên commit thư mục này).
2. Chạy thử: `npm run ios:dev` (Simulator), hoặc `npx tauri ios dev --open` để mở Xcode và chọn iPhone thật (Xcode tự quản lý chữ ký với tài khoản Developer).
3. Tạo `.ipa`: `IOS_EXPORT_METHOD=release-testing ./build-all.sh`, hoặc `npx tauri ios build --export-method <method>`. Kết quả ở `src-tauri/gen/apple/build/arm64/*.ipa` và `dist/ios/`.
4. Ký trên CI: đặt secrets `IOS_CERTIFICATE`, `IOS_CERTIFICATE_PASSWORD`, `IOS_MOBILE_PROVISION` (base64), như mô tả ở đầu `release.yml`.

**`.ipa` không cài tùy ý lên mọi iPhone được.** Các kênh phân phối hợp lệ:

| Kênh | Phù hợp | Điều kiện |
|---|---|---|
| Development / Ad Hoc (`debugging` / `release-testing`) | Nhóm nhỏ nội bộ | Đăng ký UDID từng máy (tối đa 100 thiết bị mỗi loại mỗi năm), cài qua Xcode/Apple Configurator/MDM |
| TestFlight (`app-store-connect`) | Thử nghiệm | Internal testers (thành viên App Store Connect) không cần duyệt; external testers phải qua Beta App Review |
| Custom App (Apple Business Manager) | Phát cho tổ chức | Phải qua App Review, phân phối qua ABM/MDM |
| App Store công khai | — | **Rủi ro bị từ chối cao** theo Guideline 4.2 (Minimum Functionality) vì ứng dụng chỉ bọc một website. Zyxel đã có app Nebula chính thức trên App Store, nên kênh này chỉ hợp lý nếu do chính Zyxel phát hành |
| Apple Developer Enterprise Program | Ít khả năng | Apple giới hạn đối tượng đủ điều kiện |

Hành vi riêng trên iOS: không có cửa sổ phụ (Tauri mobile chỉ có 1 cửa sổ). Vì vậy `target=_blank` và `window.open` trong allowlist được mở **tại chỗ**, còn link ngoài mở bằng Safari. Có thể vuốt từ cạnh trái màn hình để quay lại.

## 8. Checklist bảo mật và kiểm thử

**Đã có trong mã:** HTTPS bắt buộc (http:// bị chặn kể cả domain Nebula); không tắt kiểm tra TLS; allowlist so khớp chính xác hostname (đã test chặn `nebula.zyxel.com.evil…`, `evilnebula…`); iframe chỉ được phép theo tiền tố đường dẫn; không cấp quyền IPC nào; CSP chặt cho trang cục bộ; không đọc/ghi mật khẩu, cookie do WebView quản lý; log không ghi query string (tránh lộ mã OAuth); `data:`/`file:`/`javascript:` không mở được.

Ký hiệu: ✅ đã kiểm thử thực tế trong phiên này · 🔶 suy luận từ mã nguồn/tài liệu · ⬜ cần kiểm thử thủ công.

| # | Hạng mục | Linux | Windows | iOS |
|---|---|---|---|---|
| 1 | Unit test chính sách điều hướng (6 test) | ✅ | ⬜ (CI chạy) | 🔶 cùng mã |
| 2 | Build ra artifact thật | ✅ AppImage/deb/rpm | ⬜ | ⬜ |
| 3 | App khởi chạy, hiện màn hình chờ, menu | ✅ (Xvfb) | ⬜ | ⬜ |
| 4 | Không vào được Nebula (proxy chặn) → báo lỗi + nút Thử lại | ✅ | 🔶 | 🔶 |
| 5 | Mở đúng `https://nebula.zyxel.com` | ⬜ (sandbox không có mạng tới Nebula) | ⬜ | ⬜ |
| 6 | Đăng nhập Zyxel Account (email/mật khẩu, 2FA, reCAPTCHA) | ⬜ | ⬜ | ⬜ |
| 7 | Đăng nhập Apple / Microsoft Entra ID (ghi lại log `[nebula] chặn…` nếu bị chặn) | ⬜ | ⬜ | ⬜ |
| 8 | Đóng app → mở lại vẫn còn session | ⬜ | ⬜ | ⬜ |
| 9 | Đăng xuất → mở lại phải về màn hình đăng nhập | ⬜ | ⬜ | ⬜ |
| 10 | Link nội bộ, tab mới/popup trong Nebula | ⬜ | ⬜ | ⬜ |
| 11 | Link ngoài (Privacy Policy, Community) mở bằng trình duyệt hệ thống | ⬜ | ⬜ | ⬜ |
| 12 | Gõ thử `location.href="https://example.com"` từ trang → bị chặn | ⬜ | ⬜ | ⬜ |
| 13 | Ngắt mạng khi đang dùng → thanh "Mất kết nối" + Tải lại | ⬜ | ⬜ | ⬜ |
| 14 | Chứng chỉ không hợp lệ (vd. qua proxy MITM tự ký) → không tải, không có nút bỏ qua | ⬜ | ⬜ | ⬜ |
| 15 | Thay đổi kích thước cửa sổ, Quay lại/Tải lại/Trang chủ, copy/paste | ⬜ | ⬜ | ⬜ vuốt quay lại |
| 16 | Cài từ artifact, khởi chạy bản đã cài, gỡ cài đặt | ✅ `.deb` (Ubuntu 24.04); ⬜ AppImage/rpm | ⬜ | ⬜ |

**Cách tìm domain còn thiếu:** chạy `npm run dev` (hoặc chạy file thực thi từ terminal), thực hiện luồng đăng nhập, đọc các dòng `[nebula] chặn điều hướng ngoài allowlist: …`. Chỉ thêm host thực sự cần vào `authHosts`, không thêm cả domain cha.

## 9. Hạn chế còn tồn tại

1. **Đăng nhập Google không dùng được trong app.** Google chặn OAuth trong WebView nhúng (lỗi `disallowed_useragent`). `accounts.google.com` vì vậy cố ý không có trong allowlist, và app không giả mạo user-agent để lách chính sách này. Người dùng cần đăng nhập bằng Zyxel Account, Apple hoặc Microsoft.
2. **Passkey:** iOS WKWebView chỉ cho WebAuthn khi app có Associated Domains của chính `account.zyxel.com`, nên app bên thứ ba không dùng được. WebKitGTK trên Linux không hỗ trợ. WebView2 trên Windows có hỗ trợ WebAuthn/Windows Hello nhưng cần kiểm thử.
3. **Microsoft Entra ID của doanh nghiệp** có thể chuyển hướng sang IdP riêng (ADFS, Okta…) hoặc áp Conditional Access yêu cầu trình duyệt được quản lý. Khi đó cần thêm host IdP vào `authHosts`, hoặc luồng này có thể không chạy trong WebView.
4. **Trên WebKit (iOS/Linux/macOS), bộ lọc điều hướng áp dụng cả cho iframe**, nên app không tự mở trình duyệt khi gặp URL lạ (tránh việc iframe quảng cáo/analytics bật tab). Link ngoài chỉ mở ra trình duyệt khi người dùng bấm. Redirect tự động sang domain lạ sẽ bị chặn im lặng (có log).
5. Màn hình lỗi chỉ bao phủ lần tải đầu. Nếu mất mạng giữa phiên, thanh báo lỗi phụ thuộc sự kiện `offline` của WebView. Lỗi tải trang giữa phiên trên iOS có thể chỉ hiện trang trắng; khi đó dùng *Trang chủ* (desktop) hoặc mở lại app (iOS).
6. Icon hiện là **icon tạm** (đám mây chung chung). Cần thay bằng icon chính thức đã được Zyxel cho phép dùng.
7. Bundle ID `vn.ai.zyxel.nebula` là giả định. Chưa ký số Windows. Chưa notarize macOS.
8. Mã riêng iOS (cử chỉ vuốt) và cấu hình CI iOS/Windows **chưa được biên dịch thử** vì môi trường build không có macOS/Windows và không tải được Rust target iOS. Lần chạy CI đầu tiên cần được theo dõi.
9. Mỗi nền tảng dùng engine khác nhau (WebKit/Chromium), nên giao diện Nebula có thể khác chút ít giữa các bản. AppImage nặng do kèm WebKitGTK.
