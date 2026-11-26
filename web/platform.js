// Browser glue for Grumpy Pigs: fullscreen, install-as-app, offline support.
// Load after mq_js_bundle.js and before calling load().
//
// Fullscreen and the install prompt are only allowed from inside a real
// user-input event, so the game registers its on-screen buttons as
// "hotspots" each frame and this file performs the action on pointerup.
(function () {
  "use strict";

  var HOTSPOT_FULLSCREEN = 1;
  var HOTSPOT_INSTALL = 2;
  var HOTSPOT_SHARE = 3;
  var HOTSPOT_PROMPT = 4;
  var shareText = "";
  var armedPrompt = null;
  var promptAnswers = {};
  var shareResult = 0; // 1 shared, 2 copied, 3 failed
  var pendingLink = null;

  // A level link looks like ...#L1.<data>. Take it, then tidy the address bar
  // so reloading doesn't offer the same level again.
  function grabLink() {
    var h = location.hash;
    if (h && /^#L\d+\./.test(h)) {
      pendingLink = h.slice(1);
      try {
        history.replaceState(null, "", location.pathname + location.search);
      } catch (e) { /* not allowed here; harmless */ }
    }
  }
  grabLink();
  window.addEventListener("hashchange", grabLink);

  function copyText(text) {
    function fallback() {
      try {
        var ta = document.createElement("textarea");
        ta.value = text;
        ta.setAttribute("readonly", "");
        ta.style.position = "fixed";
        ta.style.opacity = "0";
        document.body.appendChild(ta);
        ta.select();
        var ok = document.execCommand("copy");
        document.body.removeChild(ta);
        shareResult = ok ? 2 : 3;
      } catch (e) {
        shareResult = 3;
      }
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(function () { shareResult = 2; }, fallback);
    } else {
      fallback();
    }
  }

  function ask(p) {
    if (!p) return;
    var answer = window.prompt(p.title, p.def);
    if (answer !== null) promptAnswers[p.id] = answer;
  }

  function share(text) {
    if (!text) return;
    if (navigator.share) {
      navigator.share({ title: "Grumpy Pigs level", url: text }).then(
        function () { shareResult = 1; },
        function (e) { if (!e || e.name !== "AbortError") copyText(text); },
      );
    } else {
      copyText(text);
    }
  }

  // Moving strings across the wasm boundary.
  function memBytes(ptr, len) {
    return new Uint8Array(wasm_memory.buffer, ptr, len);
  }
  function readStr(ptr, len) {
    return new TextDecoder().decode(memBytes(ptr, len).slice());
  }
  function utf8(str) {
    return new TextEncoder().encode(str);
  }
  function pageString(which) {
    if (which === 1) return pendingLink;
    if (which === 2) return location.origin + location.pathname;
    return null;
  }
  function storeGet(key) {
    try {
      return localStorage.getItem("grumpy-pigs." + key);
    } catch (e) {
      return null;
    }
  }
  var hotspots = [];
  var installPrompt = null;

  function standalone() {
    return (
      window.matchMedia("(display-mode: fullscreen)").matches ||
      window.matchMedia("(display-mode: standalone)").matches ||
      navigator.standalone === true
    );
  }

  function fullscreenElement() {
    return document.fullscreenElement || document.webkitFullscreenElement || null;
  }

  function canFullscreen() {
    var el = document.documentElement;
    var api = el.requestFullscreen || el.webkitRequestFullscreen;
    var enabled = document.fullscreenEnabled !== false && document.webkitFullscreenEnabled !== false;
    return !!api && enabled;
  }

  function lockLandscape() {
    try {
      var o = screen.orientation;
      if (o && o.lock) o.lock("landscape").catch(function () {});
    } catch (e) { /* not supported */ }
  }

  function toggleFullscreen() {
    try {
      if (fullscreenElement()) {
        var exit = document.exitFullscreen || document.webkitExitFullscreen;
        var p = exit && exit.call(document);
        if (p && p.catch) p.catch(function () {});
        return;
      }
      if (!canFullscreen()) return;
      var el = document.documentElement;
      var req = el.requestFullscreen || el.webkitRequestFullscreen;
      var q = req.call(el, { navigationUI: "hide" });
      if (q && q.then) q.then(lockLandscape).catch(function () {});
      else lockLandscape();
    } catch (e) { /* refused: leave the page as it is */ }
  }

  function isIosSafari() {
    var ua = navigator.userAgent;
    var ios = /iPad|iPhone|iPod/.test(ua) || (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
    return ios && !/CriOS|FxiOS|EdgiOS/.test(ua);
  }

  window.addEventListener("beforeinstallprompt", function (e) {
    e.preventDefault();
    installPrompt = e;
  });
  window.addEventListener("appinstalled", function () {
    installPrompt = null;
  });

  function promptInstall() {
    if (!installPrompt) return;
    var p = installPrompt;
    installPrompt = null;
    try {
      p.prompt();
    } catch (e) { /* ignore */ }
  }

  function onPointerUp(e) {
    var canvas = document.getElementById("glcanvas");
    if (!canvas) return;
    var r = canvas.getBoundingClientRect();
    var x = e.clientX - r.left;
    var y = e.clientY - r.top;
    for (var i = 0; i < hotspots.length; i++) {
      var h = hotspots[i];
      if (x >= h.x && x <= h.x + h.w && y >= h.y && y <= h.y + h.h) {
        if (h.id === HOTSPOT_FULLSCREEN) toggleFullscreen();
        if (h.id === HOTSPOT_INSTALL) promptInstall();
        if (h.id === HOTSPOT_SHARE) share(h.text);
        if (h.id === HOTSPOT_PROMPT) ask(h.prompt);
        return;
      }
    }
  }
  window.addEventListener("pointerup", onPointerUp, true);

  window.addEventListener("keydown", function (e) {
    if ((e.key === "f" || e.key === "F") && !e.repeat && !e.ctrlKey && !e.metaKey && !e.altKey) {
      toggleFullscreen();
    }
  });

  miniquad_add_plugin({
    name: "grumpy_pigs",
    version: 1,
    register_plugin: function (importObject) {
      var env = importObject.env;
      // 0 = unavailable, 1 = windowed (can go fullscreen), 2 = fullscreen now.
      env.gp_fullscreen_state = function () {
        if (fullscreenElement()) return 2;
        if (standalone()) return 0;
        return canFullscreen() ? 1 : 0;
      };
      // 0 = nothing to show, 1 = install prompt ready, 2 = iOS Share-sheet hint.
      env.gp_install_state = function () {
        if (standalone()) return 0;
        if (installPrompt) return 1;
        return isIosSafari() ? 2 : 0;
      };
      env.gp_hotspots_clear = function () {
        hotspots.length = 0;
      };
      // Each hotspot keeps the share text / prompt that was set just before
      // it was registered, so several Share buttons can coexist.
      env.gp_hotspot = function (id, x, y, w, h) {
        hotspots.push({ id: id, x: x, y: y, w: w, h: h, text: shareText, prompt: armedPrompt });
        armedPrompt = null;
      };
      env.gp_prompt_arm = function (id, tptr, tlen, dptr, dlen) {
        armedPrompt = { id: id, title: readStr(tptr, tlen), def: readStr(dptr, dlen) };
      };
      env.gp_prompt_len = function (id) {
        var a = promptAnswers[id];
        return a == null ? -1 : utf8(a).length;
      };
      env.gp_prompt_read = function (id, dst) {
        var b = utf8(promptAnswers[id] || "");
        delete promptAnswers[id];
        memBytes(dst, b.length).set(b);
      };
      // Small integer preferences, remembered in this browser.
      env.gp_pref_get = function (key) {
        try {
          return parseInt(localStorage.getItem("grumpy-pigs.pref." + key) || "0", 10) || 0;
        } catch (e) {
          return 0;
        }
      };
      env.gp_str_len = function (which) {
        var s = pageString(which);
        return s == null ? -1 : utf8(s).length;
      };
      env.gp_str_read = function (which, dst) {
        var b = utf8(pageString(which) || "");
        memBytes(dst, b.length).set(b);
      };
      env.gp_store_len = function (kptr, klen) {
        var v = storeGet(readStr(kptr, klen));
        return v == null ? -1 : utf8(v).length;
      };
      env.gp_store_read = function (kptr, klen, dst) {
        var b = utf8(storeGet(readStr(kptr, klen)) || "");
        memBytes(dst, b.length).set(b);
      };
      env.gp_store_write = function (kptr, klen, vptr, vlen) {
        try {
          localStorage.setItem("grumpy-pigs." + readStr(kptr, klen), readStr(vptr, vlen));
        } catch (e) { /* storage full or unavailable */ }
      };
      env.gp_link_clear = function () {
        pendingLink = null;
      };
      env.gp_share_set = function (ptr, len) {
        shareText = readStr(ptr, len);
      };
      env.gp_share_result = function () {
        var r = shareResult;
        shareResult = 0;
        return r;
      };
      env.gp_pref_set = function (key, value) {
        try {
          localStorage.setItem("grumpy-pigs.pref." + key, String(value));
        } catch (e) { /* storage unavailable: setting lasts for this session */ }
      };
    },
  });

  // Keep the screen awake while the game is open (Screen Wake Lock API). The
  // browser drops the lock whenever the page is hidden, so take it again each
  // time the game becomes visible, and on taps in case a browser wants a user
  // action first. Unsupported browsers just ignore this.
  var wakeLock = null;
  function keepAwake() {
    if (!("wakeLock" in navigator) || wakeLock || document.visibilityState !== "visible") return;
    navigator.wakeLock.request("screen").then(
      function (lock) {
        wakeLock = lock;
        lock.addEventListener("release", function () { wakeLock = null; });
      },
      function () { /* refused (e.g. battery saver); try again on the next tap */ },
    );
  }
  document.addEventListener("visibilitychange", keepAwake);
  window.addEventListener("pointerup", keepAwake);
  keepAwake();

  // Offline support. Skipped on file:// and inside sandboxed frames that forbid it.
  var secure = location.protocol === "https:" || location.hostname === "localhost";
  if ("serviceWorker" in navigator && secure) {
    window.addEventListener("load", function () {
      try {
        navigator.serviceWorker
          .register("sw.js", { updateViaCache: "none" })
          .then(function (reg) { reg.update().catch(function () {}); })
          .catch(function () {});
      } catch (e) { /* unavailable */ }
    });
  }
})();
