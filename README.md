# Grumpy Pigs

Smug wolves stole the pigs' mud. Fling furious pigs from a slingshot and knock
down the wolves' straw, stick and brick hideouts.

Rust + [macroquad](https://macroquad.rs) (rendering/input, one codebase for every
platform) + [Rapier](https://rapier.rs) (2D physics). All art is drawn
procedurally, so there are no asset files.

## Pigs

| Pig    | Power                                   |
|--------|-----------------------------------------|
| Grunt  | Plain fury                               |
| Dasher | Tap mid-air to rocket forward            |
| Boomer | Tap to explode (auto-detonates after impact) |
| Hog    | Big and heavy, made for brick             |

Controls: press anywhere, pull back, release. Mouse and touch both work.

## Run

The toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it automatically.

```sh
cargo run --release          # desktop (macOS / Windows / Linux)
cargo test --release         # physics tests, incl. a bot that proves every level is beatable
./build-web.sh               # web build -> dist/ (~1.4 MB wasm)
python3 -m http.server -d dist 8000   # then open http://localhost:8000
```

`dist/` is static files, so it can be hosted anywhere (GitHub Pages, Netlify, itch.io).

## Publish to GitHub Pages

`.github/workflows/pages.yml` checks formatting, runs pedantic clippy and the tests, builds
the web version and deploys it on every push to `main`. One-time setup:

1. Create an empty repo on GitHub and push: `git remote add origin git@github.com:<you>/grumpy-pigs.git && git push -u origin main`
2. In the repo: **Settings → Pages → Build and deployment → Source: GitHub Actions**.
3. Re-run the workflow from the **Actions** tab, or push again. The game appears at
   `https://<you>.github.io/grumpy-pigs/`.

Pages sites are public even when the repo is private (private-repo Pages needs a paid plan).

## Mobile

Not yet built or tested on devices. These are the standard macroquad routes:

- **Android:** `docker run --rm -v $(pwd):/root/src -w /root/src notfl3/cargo-apk cargo quad-apk build --release`.
  The APK lands in `target/android-artifacts/release/apk/`. Settings live in
  `[package.metadata.android]` in `Cargo.toml` (landscape, fullscreen).
- **iOS:** `rustup target add aarch64-apple-ios`, then `cargo build --release --target aarch64-apple-ios`,
  and wrap the binary in a minimal Xcode app bundle; see macroquad's
  [iOS guide](https://macroquad.rs/articles/ios/).
- **Shortcut:** the web build is already touch-friendly and mobile-viewport aware, so
  "Add to Home Screen" works today.

## Layout

- `src/sim.rs`: physics world, damage model (impact Δv), pig powers, turn flow. No rendering, so it's unit-testable.
- `src/levels.rs`: level builder helpers (`frame`, `post`, `plank`, `hill`, ...) and the 5 levels.
- `src/gfx.rs`: vector toolkit: a posable "rig" (rotation + squash/stretch) and shaded, outlined mesh primitives.
- `src/art.rs`: the animated characters. Pigs and wolves have moods (idle, aim, yell, dizzy, cheer / smug, worried, hurt, laugh), blinking, gaze tracking and squash-and-stretch.
- `src/render.rs`: scenery, material-shaded blocks, sling, HUD helpers.
- `src/main.rs`: menu, input, HUD, particles, end screens.

Lints: `clippy::pedantic` is on (see `[lints.clippy]` in `Cargo.toml`); `cargo clippy --all-targets` should stay at zero warnings.

Dev aid: `GRUMPY_SHOT=out.png GRUMPY_LEVEL=3 GRUMPY_LAUNCH=14,1.0 GRUMPY_FRAMES=300 cargo run --release`
renders a scene to a PNG and exits (`GRUMPY_AIM=deg,pow` freezes mid-aim instead).
