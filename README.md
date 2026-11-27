# Grumpy Pigs

Smug wolves stole the pigs' mud. Fling furious pigs from a slingshot and knock
down the wolves' straw, stick and brick hideouts.

Rust + [macroquad](https://macroquad.rs) (rendering/input, one codebase for every
platform) + [Rapier](https://rapier.rs) (2D physics). All art is drawn
procedurally, so there are no asset files.

## Pigs

| Pig     | Power (tap mid-air)                                   |
|---------|-------------------------------------------------------|
| Grunt   | Plain fury                                            |
| Dasher  | Rockets forward                                       |
| Boomer  | Explodes (auto-detonates after impact)                |
| Hog     | None: big and heavy, made for brick                   |
| Litter  | Splits into three piglets                             |
| Thumper | Stops dead and slams straight down                    |
| Puffer  | Inflates to twice its size, shoving structures apart  |

## Bad guys and tricks

- **Wolves**: Pups (fragile), Wolves, Helmet wolves (a cooking pot doubles their toughness)
  and the Big Bad Boss.
- **Materials**: straw, wood, brick, slippery and brittle **ice**, and **TNT** crates that
  explode when destroyed and can set each other off. Bigger crates make bigger blasts, and go off just as easily.
- **Boulders and ramps**: knock a boulder loose and let it roll downhill into a fort.
- **Fairies**: hover in mid-air holding up whatever rests on them, so structures can
  float. Hit one (with a pig, flying debris or a blast) and she flies away, dropping
  everything she was holding.
- **Portals**: come in colored pairs. Anything that flies, falls or rolls into one end
  comes out of the other at the same speed, turned to match the way the far end faces.
  Pieces too big for the opening bump into its rim instead.
- **Combos**: knock out several wolves within 1.5 s of each other for a DOUBLE / TRIPLE / MEGA KO bonus.

14 levels, from "Straw Hut" to "Big Bad Boss", "Fairy Lift" and "Portal Party".

**Easter egg:** on US Thanksgiving the game becomes *Peckish Pilgrims*: pilgrim-hatted
pigs, turkeys instead of wolves, cherubs instead of fairies and autumn trees
(`GRUMPY_HOLIDAY=thanksgiving` previews it on desktop).

## Level editor

Title screen → **My levels** → **+ New**, or tap the pencil while playing any level
to remix it (remixes credit the original author).

- **Earth / Blocks / Wolves / Pigs / Magic** palette along the bottom: drag a piece out and
  it drops onto whatever is below it (turn **Snap** off to place pieces freely).
  Blocks come in straw, wood, brick and ice, plus TNT and boulders. Magic has fairies,
  which stay exactly where you put them (great for floating platforms), and portals:
  dragging one out places both ends of a pair, each pair in a color unlike the others.
  Move and turn each end on its own; deleting either end deletes the pair.
- Every button has a picture as well as a word, so kids who can't read yet can build too.
- Tap a piece to select it: rotate in 10° steps, resize, copy, delete; tapping a
  material restyles the selected block. Drag the slingshot to move it, even onto a ledge.
- Drag empty space to pan; **+ / −** or the mouse wheel to zoom.
- Pieces that would fall over by themselves are outlined in red (the editor runs a
  couple of seconds of physics after each change).
- **Undo / Redo** step through previous versions of the level (each is a level code).
- **Test** plays it; the in-game menu button comes back to the editor.
- **Everything saves automatically.** A new level gets a random name and goes
  straight into My levels; a remix is added on its first change. If the app is
  closed mid-edit, it reopens right where you left off. **Save copy** branches off
  a copy to keep editing, leaving the current version as it is.
- Undo history (the last 50 versions) is kept on the device, so Undo still works
  after the app is closed. Tap the name or the "by …" line to rename the level or
  set your name (remembered on the device).
- **Paste link** (From friends tab) imports a level from a copied link; handy in the
  installed iPhone app, where links open in Safari instead.

## Sharing levels

Every level can be shared as a link: tap the share button while playing (or on a
saved level's card). The whole level lives in the link itself, after the `#`, so
there's no server and no account, and anyone can open it:

```
https://3noch.github.io/grumpy-pigs/#L1.AQlTdHJhdyBIdXQDC0dydW1weSBQaWdz…
```

Opening one shows the level with **Play** and **Save**; saved levels go to the
**From friends** tab (stored on that device). The format (`src/share.rs`) is
versioned and forward compatible: records with odd tags/kinds are optional and
skipped by older games, even ones are required ("needs a newer version"), and
every piece carries its own length so new fields can be added later. Positions
and sizes are in centimeters; angles cover the full circle. Each level also says where
its slingshot stands (on the ground or up on a platform).

`cargo test print_codes -- --ignored --nocapture` prints the built-in levels' codes;
`cargo run --release -- '<link>'` opens a link in the desktop build.

Controls: press on the sling, pull back, release. Drag anywhere else to look around the
level, and zoom with the + / - buttons (or the mouse wheel). Mouse and touch both work.
Left-handed? The mirror button (◀|▶, on the title screen and in-game) flips the game so the
sling is on the right and you fire to the left. The choice is remembered on that device.

## Run

The toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it automatically.

```sh
cargo run --release          # desktop (macOS / Windows / Linux)
cargo test --release         # physics tests, incl. a bot that proves every level is beatable
./build-web.sh               # web build -> dist/ (~1.4 MB wasm)
python3 -m http.server -d dist 8000   # then open http://localhost:8000
```

`dist/` is static files, so it can be hosted anywhere (GitHub Pages, Netlify, itch.io).

### Install as an app (PWA) and fullscreen

The web build is an installable Progressive Web App that works offline after the first visit:

- **Android / desktop Chrome & Edge:** an **Install** button appears on the title screen.
- **iPhone / iPad:** Safari → Share → **Add to Home Screen** (the title screen says so).
- **Screen stays on** while the game is open (Screen Wake Lock, where the browser supports it).
- **Fullscreen:** the corner button on the title screen and in-game, or press **F**.
  iPhone Safari has no fullscreen API, so there the button is hidden; installing to
  the Home Screen gives the full-screen experience instead.

Browsers only allow fullscreen and the install prompt from a real tap, so the game
registers its button areas with `web/platform.js`, which acts on the tap itself.
Caching: `build-web.sh` renames the heavy files after a hash of their contents
(`grumpy-pigs.<hash>.wasm`, ...), and `index.html` is a ~3 KB loader that names them.
The service worker (`web/sw.js`) serves fingerprinted files straight from its cache
forever and checks only the loader with the server on each launch, so a deploy shows
up on the next launch, downloads only the files that changed, and the game works offline.
Icons are rendered by the game: `./make-icons.sh` regenerates `web/icons/` (macOS).

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
- `src/levels.rs`: level builder helpers (`frame`, `post`, `plank`, `hill`, `ramp`, `tnt`, `boulder`, ...) and the 14 levels.
- `src/gfx.rs`: vector toolkit: a posable "rig" (rotation + squash/stretch) and shaded, outlined mesh primitives.
- `src/art.rs`: the animated characters. Pigs and wolves have moods (idle, aim, yell, dizzy, cheer / smug, worried, hurt, laugh), blinking, gaze tracking and squash-and-stretch.
- `src/render.rs`: scenery, material-shaded blocks, sling, HUD helpers.
- `src/main.rs`: input, HUD, particles, end screens; `src/menu.rs`: title screen, level tabs, shared-level card.
- `src/share.rs`: the level link format; `src/bank.rs`: saved levels; `src/editor.rs`: the level editor.
- `src/platform.rs` + `web/platform.js`: fullscreen and install-as-app glue.

Lints: `clippy::pedantic` is on (see `[lints.clippy]` in `Cargo.toml`); `cargo clippy --all-targets` should stay at zero warnings.

Dev aids (desktop build, environment variables):

- `GRUMPY_SHOT=out.png GRUMPY_FRAMES=300` renders frames then saves a PNG and exits.
- `GRUMPY_LEVEL=3`, `GRUMPY_LAUNCH=deg,pow`, `GRUMPY_TAP=secs`, `GRUMPY_AIM=deg,pow`: play a level and shoot.
- `GRUMPY_EDIT=n` (0 = new), `GRUMPY_EDIT_SELECT=i`, `GRUMPY_EDIT_CAT=earth|blocks|wolves|pigs|magic`: open the editor.
- `GRUMPY_INPUT="10:p:435:659;14:m:600:350;18:r:640:300"`: replay pointer events
  (frame:press/move/release:x:y in screen points), e.g. to test editor drags.
- `GRUMPY_MIRROR=1`, `GRUMPY_TAB=mine|friends`, `GRUMPY_LAUNCHER=x,y`.
- Set `HOME` to a scratch folder to keep test runs out of your real `~/.grumpy-pigs`.

## License

Grumpy Pigs is free to play, copy, change and share for any **noncommercial** purpose
(personal use, hobby projects, study, schools, charities), under the
[PolyForm Noncommercial License 1.0.0](LICENSE.md). You may not sell it or anything
made from it. The libraries it uses (macroquad, rapier and their dependencies) keep
their own licenses.
