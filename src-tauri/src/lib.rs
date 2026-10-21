//! Nebula wrapper: một cửa sổ WebView chỉ phục vụ Zyxel Nebula.
//!
//! Nguyên tắc bảo mật:
//! - Không cấp quyền IPC nào (capabilities/default.json rỗng): trang web không gọi được Rust.
//! - Mọi điều hướng đi qua `navigation_decision` (allowlist hostname, chỉ HTTPS).
//! - Liên kết ngoài mở bằng trình duyệt hệ thống, không chạy trong ứng dụng.
//! - Không bỏ qua kiểm tra TLS, không lưu/đọc mật khẩu; cookie/session do WebView hệ thống quản lý.

use serde::Deserialize;
use std::sync::OnceLock;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// Scheme nội bộ: script trong trang đổi click vào liên kết ngoài thành `nebula-external:<url>`
/// để Rust phân biệt "người dùng bấm liên kết ngoài" với iframe/redirect tự động.
const EXTERNAL_SCHEME: &str = "nebula-external";

/// Nội dung nhúng (iframe) đã quan sát thấy trên trang đăng nhập/trang chủ.
/// Cho phép theo TIỀN TỐ ĐƯỜNG DẪN, không cho cả domain.
/// Cần thiết vì trên WebKit (iOS/macOS/Linux) bộ lọc điều hướng áp dụng cho cả iframe.
const EMBED_PREFIXES: &[&str] = &[
    "https://www.google.com/recaptcha/",       // reCAPTCHA Enterprise trên account.zyxel.com
    "https://www.recaptcha.net/recaptcha/",    // domain thay thế của reCAPTCHA
    "https://www.youtube-nocookie.com/embed/", // video giới thiệu trên trang chủ Nebula
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NebulaConfig {
    start_url: String,
    main_hosts: Vec<String>,
    #[serde(default)]
    auth_hosts: Vec<String>,
    #[serde(default = "default_external")]
    external_links: String,
}

fn default_external() -> String {
    "open-in-browser".into()
}

fn config() -> &'static NebulaConfig {
    static CFG: OnceLock<NebulaConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        let cfg: NebulaConfig = serde_json::from_str(include_str!("../../nebula.config.json"))
            .expect("nebula.config.json không hợp lệ");
        let start = Url::parse(&cfg.start_url).expect("startUrl không hợp lệ");
        assert_eq!(start.scheme(), "https", "startUrl bắt buộc dùng HTTPS");
        assert!(
            cfg.main_hosts.iter().any(|h| Some(h.as_str()) == start.host_str()),
            "host của startUrl phải nằm trong mainHosts"
        );
        cfg
    })
}

fn host_allowed(host: &str) -> bool {
    let c = config();
    c.main_hosts
        .iter()
        .chain(c.auth_hosts.iter())
        .any(|h| h.eq_ignore_ascii_case(host))
}

/// URL trang khởi chạy cục bộ (khác nhau theo nền tảng do cơ chế custom protocol).
fn local_index_url() -> Url {
    #[cfg(any(windows, target_os = "android"))]
    let s = "http://tauri.localhost/index.html";
    #[cfg(not(any(windows, target_os = "android")))]
    let s = "tauri://localhost/index.html";
    Url::parse(s).unwrap()
}

fn is_local_app_origin(u: &Url) -> bool {
    match (u.scheme(), u.host_str()) {
        ("tauri", Some("localhost")) => true,
        ("http" | "https", Some("tauri.localhost")) => true,
        _ => false,
    }
}

#[derive(Debug, PartialEq)]
enum Decision {
    /// Cho phép tải bên trong WebView.
    Allow,
    /// Chặn và mở URL (đã giải mã) bằng trình duyệt hệ thống.
    BlockOpenExternal(Url),
    /// Chặn im lặng (ghi log để kiểm thử phát hiện domain còn thiếu).
    Block,
}

fn is_openable_externally(u: &Url) -> bool {
    matches!(u.scheme(), "https" | "http" | "mailto" | "tel")
}

fn navigation_decision(u: &Url) -> Decision {
    // 1) Liên kết ngoài do người dùng bấm (được script trong trang đánh dấu).
    if u.scheme() == EXTERNAL_SCHEME {
        let encoded = u.as_str().trim_start_matches(EXTERNAL_SCHEME).trim_start_matches(':');
        let decoded = percent_decode(encoded);
        return match Url::parse(&decoded) {
            Ok(target) if is_openable_externally(&target) => {
                if config().external_links == "block" {
                    Decision::Block
                } else {
                    Decision::BlockOpenExternal(target)
                }
            }
            _ => Decision::Block,
        };
    }
    // 2) Trang khởi chạy cục bộ của ứng dụng.
    if is_local_app_origin(u) {
        return Decision::Allow;
    }
    // 3) Tài liệu rỗng / blob do chính trang tạo ra (popup, iframe, tải file).
    if matches!(u.as_str(), "about:blank" | "about:srcdoc") || u.scheme() == "blob" {
        return Decision::Allow;
    }
    // 4) Chỉ HTTPS + host trong allowlist.
    if u.scheme() == "https" {
        if let Some(h) = u.host_str() {
            if host_allowed(h) {
                return Decision::Allow;
            }
        }
        if EMBED_PREFIXES.iter().any(|p| u.as_str().starts_with(p)) {
            return Decision::Allow;
        }
    }
    Decision::Block
}

/// Giải mã percent-encoding tối thiểu (tránh thêm dependency).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn open_external<R: Runtime>(app: &AppHandle<R>, target: &Url) {
    if let Err(e) = app.opener().open_url(target.as_str(), None::<&str>) {
        eprintln!("[nebula] không mở được trình duyệt hệ thống: {e}");
    }
}

/// Script chạy ở main frame của MỌI trang (cả trang cục bộ lẫn Nebula). Không dùng IPC.
fn init_script() -> String {
    let c = config();
    let hosts: Vec<&String> = c.main_hosts.iter().chain(c.auth_hosts.iter()).collect();
    let mobile = cfg!(mobile);
    format!(
        r#"(function () {{
  "use strict";
  var START = {start};
  var HOSTS = {hosts};
  var MOBILE = {mobile};
  var EXT = "{ext}:";
  var isLocal = location.protocol === "tauri:" || location.hostname === "tauri.localhost";
  if (isLocal) {{ window.__NEBULA_START_URL__ = START; return; }}

  function allowed(u) {{
    return u.protocol === "https:" && HOSTS.indexOf(u.hostname.toLowerCase()) !== -1;
  }}
  function openExternal(href) {{ location.href = EXT + encodeURIComponent(href); }}

  // Liên kết: ngoài allowlist -> trình duyệt hệ thống; _blank trên iOS -> mở tại chỗ.
  document.addEventListener("click", function (ev) {{
    if (ev.defaultPrevented || ev.button !== 0) return;
    var a = ev.target && ev.target.closest ? ev.target.closest("a[href]") : null;
    if (!a || a.hasAttribute("download")) return;
    var u;
    try {{ u = new URL(a.href, location.href); }} catch (e) {{ return; }}
    if (u.protocol === "javascript:" || (u.origin === location.origin && u.hash && u.pathname === location.pathname)) return;
    if (!allowed(u)) {{
      if (/^(https?|mailto|tel):$/.test(u.protocol)) {{ ev.preventDefault(); openExternal(u.href); }}
      return;
    }}
    if (MOBILE && a.target && a.target !== "_self") {{ ev.preventDefault(); location.href = u.href; }}
  }}, true);

  // iOS không hỗ trợ cửa sổ mới: window.open -> mở tại chỗ hoặc ra trình duyệt.
  if (MOBILE) {{
    window.open = function (href) {{
      if (!href) return null;
      var u;
      try {{ u = new URL(href, location.href); }} catch (e) {{ return null; }}
      if (allowed(u)) location.href = u.href; else openExternal(u.href);
      return null;
    }};
  }}

  // Thông báo mất mạng + nút tải lại (Shadow DOM để không ảnh hưởng CSS của Nebula).
  var bar = null;
  function showBar(offline) {{
    if (!document.body) return;
    if (!bar) {{
      var host = document.createElement("div");
      host.style.cssText = "position:fixed;left:0;right:0;bottom:0;z-index:2147483647";
      var root = host.attachShadow({{ mode: "closed" }});
      root.innerHTML = '<style>div{{display:flex;gap:12px;align-items:center;justify-content:center;padding:10px 16px;font:14px system-ui,sans-serif;background:#1d2733;color:#fff}}button{{border:0;border-radius:6px;padding:6px 14px;background:#4ea3e8;color:#fff;font:inherit;cursor:pointer}}</style><div><span></span><button type="button">Tải lại</button></div>';
      root.querySelector("button").onclick = function () {{ location.reload(); }};
      bar = {{ host: host, text: root.querySelector("span") }};
      document.body.appendChild(host);
    }}
    bar.text.textContent = offline ? "Mất kết nối mạng." : "Đã có mạng trở lại.";
    bar.host.style.display = "block";
  }}
  window.addEventListener("offline", function () {{ showBar(true); }});
  window.addEventListener("online", function () {{ showBar(false); }});
}})();"#,
        start = serde_json::to_string(&c.start_url).unwrap(),
        hosts = serde_json::to_string(&hosts).unwrap(),
        mobile = mobile,
        ext = EXTERNAL_SCHEME,
    )
}

/// Tạo cửa sổ (chính hoặc popup đăng nhập) với cùng chính sách điều hướng.
fn build_window<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    url: WebviewUrl,
    popup_features: Option<tauri::webview::NewWindowFeatures>,
) -> tauri::Result<tauri::WebviewWindow<R>> {
    let nav_app = app.clone();
    let mut b = WebviewWindowBuilder::new(app, label, url)
        .initialization_script(init_script())
        .on_navigation(move |u| match navigation_decision(u) {
            Decision::Allow => true,
            Decision::BlockOpenExternal(target) => {
                open_external(&nav_app, &target);
                false
            }
            Decision::Block => {
                eprintln!("[nebula] chặn điều hướng ngoài allowlist: {}", redact(u));
                false
            }
        });

    #[cfg(desktop)]
    {
        let popup_app = app.clone();
        b = b
            .title("Zyxel Nebula")
            .on_document_title_changed(|w, title| {
                let _ = w.set_title(if title.is_empty() { "Zyxel Nebula" } else { &title });
            })
            .on_new_window(move |u, features| {
                let ok = u.as_str() == "about:blank"
                    || matches!(navigation_decision(&u), Decision::Allow);
                if !ok {
                    match navigation_decision(&u) {
                        Decision::BlockOpenExternal(t) => open_external(&popup_app, &t),
                        _ if u.scheme() == "https" || u.scheme() == "http" => {
                            if config().external_links != "block" {
                                open_external(&popup_app, &u)
                            }
                        }
                        _ => {}
                    }
                    return NewWindowResponse::Deny;
                }
                // Popup thuộc allowlist (ví dụ cửa sổ SSO cần window.opener): tạo cửa sổ con.
                static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
                let label = format!("popup-{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
                match build_window(
                    &popup_app,
                    &label,
                    WebviewUrl::External("about:blank".parse().unwrap()),
                    Some(features),
                ) {
                    Ok(window) => NewWindowResponse::Create { window },
                    Err(e) => {
                        eprintln!("[nebula] không tạo được popup: {e}");
                        NewWindowResponse::Deny
                    }
                }
            });
        b = match popup_features {
            Some(f) => b.window_features(f).focused(true),
            None => b.inner_size(1280.0, 800.0).min_inner_size(800.0, 560.0).center(),
        };
    }
    #[cfg(mobile)]
    let _ = popup_features;

    let w = b.build()?;

    // iOS: bật vuốt cạnh màn hình để quay lại/tiến tới (WKWebView).
    #[cfg(target_os = "ios")]
    {
        let _ = w.with_webview(|pw| unsafe {
            // pw.inner() là con trỏ WKWebView; chạy trên main thread.
            let wk = &*(pw.inner() as *const objc2::runtime::AnyObject);
            let _: () = objc2::msg_send![wk, setAllowsBackForwardNavigationGestures: true];
        });
    }
    Ok(w)
}

/// Không ghi query string ra log (có thể chứa mã OAuth/token).
fn redact(u: &Url) -> String {
    let mut c = u.clone();
    c.set_query(None);
    c.set_fragment(None);
    c.to_string()
}

#[cfg(desktop)]
fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
    let back = MenuItemBuilder::with_id("back", "Quay lại").accelerator("Alt+Left").build(app)?;
    let fwd = MenuItemBuilder::with_id("forward", "Tiến tới").accelerator("Alt+Right").build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "Tải lại").accelerator("CmdOrCtrl+R").build(app)?;
    let home = MenuItemBuilder::with_id("home", "Trang chủ Nebula").accelerator("Alt+Home").build(app)?;
    let nav = SubmenuBuilder::new(app, "Điều hướng")
        .item(&back)
        .item(&fwd)
        .item(&reload)
        .item(&home)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Thoát"))?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Sửa")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    MenuBuilder::new(app).item(&nav).item(&edit).build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = config(); // kiểm tra cấu hình ngay khi khởi động

    let builder = tauri::Builder::default().plugin(
        // Chỉ dùng API Rust của opener; KHÔNG chèn script chặn click và KHÔNG cấp quyền JS.
        tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build(),
    );

    #[cfg(desktop)]
    let builder = builder
        .menu(|app| build_menu(app))
        .on_menu_event(|app, ev| {
            let Some(w) = app
                .webview_windows()
                .into_values()
                .find(|w| w.is_focused().unwrap_or(false))
                .or_else(|| app.get_webview_window("main"))
            else {
                return;
            };
            let _ = match ev.id().as_ref() {
                "back" => w.eval("history.back()"),
                "forward" => w.eval("history.forward()"),
                "reload" => w.eval("location.reload()"),
                "home" => w.navigate(local_index_url()),
                _ => Ok(()),
            };
        });

    builder
        .setup(|app| {
            build_window(app.handle(), "main", WebviewUrl::App("index.html".into()), None)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("lỗi khi chạy ứng dụng Nebula");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decision {
        navigation_decision(&Url::parse(s).unwrap())
    }

    #[test]
    fn allows_nebula_and_auth() {
        assert_eq!(d("https://nebula.zyxel.com/cc/"), Decision::Allow);
        assert_eq!(d("https://account.zyxel.com/users/sign_in"), Decision::Allow);
        assert_eq!(d("https://NEBULA.zyxel.com/"), Decision::Allow);
    }

    #[test]
    fn allows_local_and_blank() {
        assert_eq!(d("tauri://localhost/index.html"), Decision::Allow);
        assert_eq!(d("http://tauri.localhost/index.html"), Decision::Allow);
        assert_eq!(d("about:blank"), Decision::Allow);
    }

    #[test]
    fn blocks_http_and_lookalikes() {
        assert_eq!(d("http://nebula.zyxel.com/"), Decision::Block);
        assert_eq!(d("https://nebula.zyxel.com.evil.example/"), Decision::Block);
        assert_eq!(d("https://evilnebula.zyxel.com/"), Decision::Block);
        assert_eq!(d("https://www.zyxel.com/"), Decision::Block);
        assert_eq!(d("https://accounts.google.com/o/oauth2"), Decision::Block);
        assert_eq!(d("data:text/html,hi"), Decision::Block);
        assert_eq!(d("file:///etc/passwd"), Decision::Block);
    }

    #[test]
    fn embeds_by_path_prefix_only() {
        assert_eq!(d("https://www.google.com/recaptcha/enterprise/anchor?k=x"), Decision::Allow);
        assert_eq!(d("https://www.google.com/search?q=x"), Decision::Block);
        assert_eq!(d("https://www.youtube-nocookie.com/embed/abc"), Decision::Allow);
        assert_eq!(d("https://www.youtube.com/watch?v=abc"), Decision::Block);
    }

    #[test]
    fn external_marker() {
        let u = format!("{EXTERNAL_SCHEME}:{}", "https%3A%2F%2Fwww.zyxel.com%2Fglobal%2Fen%2Fprivacy-policy");
        assert_eq!(
            d(&u),
            Decision::BlockOpenExternal(Url::parse("https://www.zyxel.com/global/en/privacy-policy").unwrap())
        );
        let bad = format!("{EXTERNAL_SCHEME}:{}", "file%3A%2F%2F%2Fetc%2Fpasswd");
        assert_eq!(d(&bad), Decision::Block);
        let js = format!("{EXTERNAL_SCHEME}:{}", "javascript%3Aalert(1)");
        assert_eq!(d(&js), Decision::Block);
    }

    #[test]
    fn init_script_embeds_config() {
        let s = init_script();
        assert!(s.contains("nebula.zyxel.com"));
        assert!(s.contains("account.zyxel.com"));
    }
}
