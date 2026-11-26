// Grumpy Pigs service worker.
//
// build-web.sh fingerprints the heavy files (game, JS) with a hash of their
// contents, so their URLs never change and they're served straight from cache.
// Everything else (the tiny index.html loader, manifest, icons) is checked with
// the server on every launch, so a new deploy shows up on the very next launch.
// Offline, everything falls back to the cache.

const BUILD = "__BUILD__"; // replaced by build-web.sh
const ASSETS = []; // replaced by build-web.sh with this build's fingerprinted files
const SHELL = ["./", "manifest.webmanifest", "icons/icon-192.png", "icons/icon-512.png"];
const CACHE = "grumpy-pigs-" + BUILD;
const FINGERPRINTED = /\.[0-9a-f]{10}\.(js|wasm)$/;

// Reuse a copy from any older cache when a file didn't change between builds.
function fetchForCache(url) {
  return caches
    .match(url)
    .then((hit) => hit || fetch(new Request(url, { cache: "no-cache" })));
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) =>
        Promise.all(
          ASSETS.map((url) => fetchForCache(url).then((res) => cache.put(url, res))).concat(
            SHELL.map((url) => fetch(new Request(url, { cache: "no-cache" })).then((res) => cache.put(url, res))),
          ),
        ),
      )
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((k) => k.startsWith("grumpy-pigs-") && k !== CACHE).map((k) => caches.delete(k))),
      )
      .then(() => self.clients.claim()),
  );
});

function remember(req, res) {
  if (res.ok) {
    const copy = res.clone();
    caches.open(CACHE).then((cache) => cache.put(req, copy));
  }
  return res;
}

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== self.location.origin) return;

  if (FINGERPRINTED.test(url.pathname)) {
    // Immutable: cache first.
    event.respondWith(caches.match(req).then((hit) => hit || fetch(req).then((res) => remember(req, res))));
    return;
  }

  // Loader, manifest, icons: always ask the server, fall back to cache offline.
  // A navigation request can't be cloned with new options, so refetch by URL.
  const fresh =
    req.mode === "navigate"
      ? fetch(req.url, { cache: "no-cache", credentials: "same-origin" })
      : fetch(req, { cache: "no-cache" });
  event.respondWith(
    fresh
      .then((res) => remember(req, res))
      .catch(() =>
        caches
          .match(req, { ignoreSearch: true })
          .then((hit) => hit || (req.mode === "navigate" ? caches.match("./") : undefined))
          .then((hit) => hit || Response.error()),
      ),
  );
});
