//! Low-level vector drawing: a posable local frame ("rig") plus shaded,
//! outlined primitives built from triangle meshes.

use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;

use crate::render::View;
use crate::sim::V2;

pub type P = (f32, f32);

const GLOW_VERTEX: &str = r"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}";

const GLOW_FRAGMENT: &str = r"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;
uniform sampler2D Texture;
void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}";

thread_local! {
    /// Draws by adding light to what's behind, rather than covering it.
    static GLOW: Option<Material> = {
        use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams, ShaderSource};
        let add = BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::One,
        );
        load_material(
            ShaderSource::Glsl {
                vertex: GLOW_VERTEX,
                fragment: GLOW_FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(add),
                    ..PipelineParams::default()
                },
                ..MaterialParams::default()
            },
        )
        .ok()
    };
}

/// Runs `draw` so that everything it draws glows: it brightens the scene
/// behind it, which stands out most against dark skies.
pub fn glow(draw: impl FnOnce()) {
    GLOW.with(|material| {
        if let Some(m) = material {
            gl_use_material(m);
            draw();
            gl_use_default_material();
        } else {
            draw();
        }
    });
}

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        a,
    )
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// A color from its place on the color wheel (degrees), saturation and
/// brightness (both 0..1).
pub fn hsv(hue: f32, sat: f32, val: f32) -> Color {
    let h = hue.rem_euclid(360.0) / 60.0;
    let c = val * sat;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match count(h, 5) {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = val - c;
    Color::new(r + m, g + m, b + m, 1.0)
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a)
}

/// Deterministic pseudo-random in [0, 1) from a seed and a salt.
pub fn hash(seed: u32, salt: u32) -> f32 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65536.0
}

/// Quadratic Bézier sampled into `n + 1` points.
pub fn bez(a: P, c: P, b: P, n: usize) -> Vec<P> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            (
                u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
            )
        })
        .collect()
}

/// A local drawing frame: `scale` world units per local unit, rotated by
/// `angle`, then squashed/stretched (area-preserving) along a world direction.
#[derive(Clone, Copy)]
pub struct Rig {
    view: View,
    c: V2,
    cos: f32,
    sin: f32,
    scale: f32,
    m: [f32; 4],
}

impl Rig {
    pub fn new(view: View, c: V2, angle: f32, scale: f32) -> Self {
        Rig {
            view,
            c,
            cos: angle.cos(),
            sin: angle.sin(),
            scale,
            m: [1.0, 0.0, 0.0, 1.0],
        }
    }

    /// Stretch by `1 + s` along `dir` and shrink by the inverse across it.
    #[must_use]
    pub fn stretched(mut self, dir: V2, s: f32) -> Self {
        let d = dir.normalize_or(V2::Y);
        let a = (1.0 + s).max(0.3);
        let b = 1.0 / a;
        let off = (a - b) * d.x * d.y;
        self.m = [
            a * d.x * d.x + b * d.y * d.y,
            off,
            off,
            a * d.y * d.y + b * d.x * d.x,
        ];
        self
    }

    /// Local point → world point.
    pub fn world(&self, x: f32, y: f32) -> V2 {
        let (x, y) = (x * self.scale, y * self.scale);
        let rx = x * self.cos - y * self.sin;
        let ry = x * self.sin + y * self.cos;
        let [a, b, c, d] = self.m;
        V2::new(self.c.x + a * rx + b * ry, self.c.y + c * rx + d * ry)
    }

    /// Local point → screen pixel.
    pub fn p(&self, x: f32, y: f32) -> Vec2 {
        self.view.v(self.world(x, y))
    }

    /// Screen pixels per local unit.
    pub fn px(&self) -> f32 {
        self.view.s * self.scale
    }

    /// World-space direction expressed in this rig's (unrotated) local axes.
    pub fn local_dir(&self, d: V2) -> V2 {
        V2::new(
            d.x * self.cos + d.y * self.sin,
            -d.x * self.sin + d.y * self.cos,
        )
    }

    /// Triangle fan from `center` to a closed `ring`, with a color gradient.
    pub fn fan(&self, center: P, ring: &[P], inner: Color, outer: Color) {
        if ring.len() < 2 || ring.len() > 4000 {
            return;
        }
        let mut vertices = Vec::with_capacity(ring.len() + 1);
        let c = self.p(center.0, center.1);
        vertices.push(Vertex::new(c.x, c.y, 0.0, 0.0, 0.0, inner));
        for &(x, y) in ring {
            let q = self.p(x, y);
            vertices.push(Vertex::new(q.x, q.y, 0.0, 0.0, 0.0, outer));
        }
        let n = u16::try_from(ring.len()).unwrap_or(u16::MAX);
        let mut indices = Vec::with_capacity(ring.len() * 3);
        for i in 0..n {
            indices.extend_from_slice(&[0, i + 1, (i + 1) % n + 1]);
        }
        draw_mesh(&Mesh {
            vertices,
            indices,
            texture: None,
        });
    }

    /// Closed shape whose radius is `f(theta)`, shaded from a light point.
    pub fn radial(
        &self,
        center: P,
        f: impl Fn(f32) -> f32,
        grow: f32,
        inner: Color,
        outer: Color,
        light: P,
    ) {
        let n = 40;
        let ring: Vec<P> = (0..n)
            .map(|i| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU;
                let r = f(t) + grow;
                (center.0 + t.cos() * r, center.1 + t.sin() * r)
            })
            .collect();
        self.fan(
            (center.0 + light.0, center.1 + light.1),
            &ring,
            inner,
            outer,
        );
    }

    pub fn ellipse_ring(cx: f32, cy: f32, rx: f32, ry: f32, rot: f32) -> Vec<P> {
        let (s, c) = rot.sin_cos();
        (0..28)
            .map(|i| {
                let t = i as f32 / 28.0 * std::f32::consts::TAU;
                let (x, y) = (t.cos() * rx, t.sin() * ry);
                (cx + x * c - y * s, cy + x * s + y * c)
            })
            .collect()
    }

    pub fn ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, col: Color) {
        self.fan((cx, cy), &Self::ellipse_ring(cx, cy, rx, ry, rot), col, col);
    }

    pub fn circle(&self, cx: f32, cy: f32, r: f32, col: Color) {
        self.ellipse(cx, cy, r, r, 0.0, col);
    }

    /// Ellipse with a dark outline and a highlight-offset gradient fill.
    #[allow(clippy::too_many_arguments)]
    pub fn blob(
        &self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rot: f32,
        light: Color,
        base: Color,
        outline: Option<(Color, f32)>,
    ) {
        if let Some((oc, ow)) = outline {
            self.ellipse(cx, cy, rx + ow, ry + ow, rot, oc);
        }
        let ring = Self::ellipse_ring(cx, cy, rx, ry, rot);
        self.fan((cx - rx * 0.3, cy + ry * 0.35), &ring, light, base);
    }

    /// Convex polygon, optional outline drawn as a stroke underneath.
    pub fn poly(&self, pts: &[P], col: Color, outline: Option<(Color, f32)>) {
        if pts.is_empty() {
            return;
        }
        if let Some((oc, ow)) = outline {
            let mut closed = pts.to_vec();
            closed.push(pts[0]);
            self.stroke(&closed, ow * 2.0, oc);
        }
        let n = pts.len() as f32;
        let cx = pts.iter().map(|p| p.0).sum::<f32>() / n;
        let cy = pts.iter().map(|p| p.1).sum::<f32>() / n;
        self.fan((cx, cy), pts, col, col);
    }

    /// Polyline with round joints, `w` in local units.
    pub fn stroke(&self, pts: &[P], w: f32, col: Color) {
        let px = w * self.px();
        for seg in pts.windows(2) {
            let a = self.p(seg[0].0, seg[0].1);
            let b = self.p(seg[1].0, seg[1].1);
            draw_line(a.x, a.y, b.x, b.y, px, col);
        }
        if px > 2.0 {
            for &(x, y) in pts {
                let q = self.p(x, y);
                draw_circle(q.x, q.y, px / 2.0, col);
            }
        }
    }

    /// Axis-aligned local rectangle with a vertical gradient.
    pub fn rect_grad(&self, x0: f32, y0: f32, x1: f32, y1: f32, top: Color, bottom: Color) {
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let cols = [bottom, bottom, top, top];
        let vertices = corners
            .iter()
            .zip(cols)
            .map(|(&(x, y), c)| {
                let q = self.p(x, y);
                Vertex::new(q.x, q.y, 0.0, 0.0, 0.0, c)
            })
            .collect();
        draw_mesh(&Mesh {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: None,
        });
    }
}

/// Converts a float to a loop count, clamped to `0..=max`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn count(x: f32, max: u32) -> u32 {
    x.clamp(0.0, max as f32) as u32
}

/// Animated blink amount (0 open .. 1 closed), staggered per seed.
pub fn blink(t: f32, seed: u32) -> f32 {
    let period = 2.6 + hash(seed, 1) * 2.4;
    let phase = (t + hash(seed, 2) * 10.0) % period;
    let d = 0.16;
    if phase < d {
        1.0 - (phase / d * 2.0 - 1.0).abs()
    } else {
        0.0
    }
}
