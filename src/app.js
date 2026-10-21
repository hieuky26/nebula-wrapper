// Màn hình khởi chạy cục bộ: kiểm tra kết nối rồi chuyển sang Nebula.
// Trang này KHÔNG dùng IPC của Tauri; URL đích được Rust tiêm vào window.__NEBULA_START_URL__
// (chỉ trên origin cục bộ của ứng dụng).
(function () {
  "use strict";
  var START = window.__NEBULA_START_URL__ || "https://nebula.zyxel.com/";
  var TIMEOUT_MS = 15000;
  // Thời gian chờ trang Nebula bắt đầu hiển thị sau khi chuyển hướng. Nếu điều hướng thất bại
  // (proxy/tường lửa chặn, captive portal, lỗi TLS), WebView giữ nguyên trang này -> báo lỗi.
  var NAV_TIMEOUT_MS = 25000;

  var loading = document.getElementById("loading");
  var error = document.getElementById("error");
  var detail = document.getElementById("err-detail");
  var retryBtn = document.getElementById("retry");

  function showError(msg) {
    loading.hidden = true;
    error.hidden = false;
    detail.textContent = msg;
    retryBtn.focus();
  }

  function connect() {
    error.hidden = true;
    loading.hidden = false;

    if (navigator.onLine === false) {
      showError("Thiết bị đang ngoại tuyến. Kết nối mạng rồi bấm Thử lại.");
      return;
    }

    var ctrl = typeof AbortController === "function" ? new AbortController() : null;
    var timer = setTimeout(function () { if (ctrl) ctrl.abort(); }, TIMEOUT_MS);

    // no-cors: chỉ kiểm tra máy chủ có phản hồi qua HTTPS hợp lệ hay không.
    // Lỗi DNS, mất mạng hoặc chứng chỉ TLS không hợp lệ đều làm fetch thất bại.
    fetch(START, { mode: "no-cors", cache: "no-store", credentials: "omit", signal: ctrl ? ctrl.signal : undefined })
      .then(function () {
        clearTimeout(timer);
        setTimeout(function () {
          showError("Trang Nebula không tải được. Có thể mạng đang bị proxy/tường lửa chặn, cần đăng nhập Wi-Fi (captive portal) hoặc chứng chỉ HTTPS không hợp lệ.");
        }, NAV_TIMEOUT_MS);
        window.location.replace(START);
      })
      .catch(function (e) {
        clearTimeout(timer);
        var timedOut = e && e.name === "AbortError";
        showError(timedOut
          ? "Hết thời gian chờ (" + TIMEOUT_MS / 1000 + " giây). Máy chủ không phản hồi."
          : "Không tải được Nebula: mất mạng, bị chặn bởi tường lửa hoặc chứng chỉ HTTPS không hợp lệ.");
      });
  }

  retryBtn.addEventListener("click", connect);
  window.addEventListener("online", function () { if (!error.hidden) connect(); });
  connect();
})();
