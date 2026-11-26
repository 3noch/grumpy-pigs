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
      env.gp_hotspot = function (id, x, y, w, h) {
        hotspots.push({ id: id, x: x, y: y, w: w, h: h });
      };
    },
  });

  // Offline support. Skipped on file:// and inside sandboxed frames that forbid it.
  var secure = location.protocol === "https:" || location.hostname === "localhost";
  if ("serviceWorker" in navigator && secure) {
    window.addEventListener("load", function () {
      try {
        navigator.serviceWorker.register("sw.js").catch(function () {});
      } catch (e) { /* unavailable */ }
    });
  }
})();
