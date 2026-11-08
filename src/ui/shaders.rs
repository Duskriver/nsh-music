//! An ambient shader background: soft colored light that drifts, tinted by
//! the track that is playing. It shows NSH drawing a live WGSL shader behind
//! native views.

use std::sync::OnceLock;

use nsh_base::theme;
use nsh_core::{color::Hsla, prelude::*, shader::Shader};

// `fn shade(uv)` draws three soft orbs of light on slow orbits, their colors
// the app's, added and rolled off so they never clip. `nsh.time` moves them,
// so the shader draws every frame.
const AURORA: &str = r#"
struct Uniforms { a: vec4f, b: vec4f, c: vec4f, intensity: f32 }
fn orb(p: vec2f, center: vec2f, size: f32) -> f32 {
  let d = p - center;
  return exp(-dot(d, d) / (size * size));
}
fn shade(uv: vec2f) -> vec4f {
  let aspect = vec2f(nsh.size.x / max(nsh.size.y, 1.0), 1.0);
  let p = uv * aspect;
  let t = nsh.time * 0.22;
  var light = vec3f(0.0);
  light += uniforms.a.rgb * orb(p, vec2f(0.28 + 0.16 * sin(t * 0.8), 0.30 + 0.20 * cos(t * 0.6)) * aspect, 0.55) * 1.15;
  light += uniforms.b.rgb * orb(p, vec2f(0.72 + 0.18 * cos(t * 0.5), 0.62 + 0.22 * sin(t * 0.7 + 1.0)) * aspect, 0.60) * 1.1;
  light += uniforms.c.rgb * orb(p, vec2f(0.52 + 0.24 * sin(t * 0.4 + 2.0), 0.86 + 0.12 * cos(t * 0.5)) * aspect, 0.55) * 0.9;
  let base = vec3f(0.015, 0.015, 0.02);
  let color = base + (vec3f(1.0) - exp(-light * uniforms.intensity)) * 0.92;
  return vec4f(color, 1.0);
}"#;

/// The compiled aurora program, or `None` where shaders are unavailable.
fn program() -> Option<Shader> {
    static PROGRAM: OnceLock<Option<Shader>> = OnceLock::new();
    PROGRAM
        .get_or_init(|| match Shader::compile(AURORA) {
            Ok(shader) => Some(shader),
            Err(error) => {
                log::warn!("the aurora shader did not compile: {error:?}");
                None
            }
        })
        .clone()
}

/// Three colors to tint the aurora with, derived from a track's URI so each
/// song has its own light; falls back to the theme before anything plays.
pub fn colors_for(uri: &str) -> [Hsla; 3] {
    if uri.is_empty() {
        let t = theme();
        return [color(t.primary), color(t.chart_2), color(t.chart_4)];
    }
    // A stable hue from the id, then a triad around it.
    let hash = uri.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let base = (hash % 360) as f32 / 360.0;
    let at = |offset: f32, light: f32| Hsla { h: (base + offset).fract(), s: 0.62, l: light, a: 1.0 };
    [at(0.0, 0.5), at(0.12, 0.42), at(0.6, 0.38)]
}

/// The aurora behind `child`, tinted by `colors`. Without shader support it
/// draws a quiet gradient of the first color instead.
pub fn ambient(colors: impl Fn() -> [Hsla; 3] + Copy + 'static, child: impl IntoElement) -> View {
    let base = view().absolute().inset_0();
    let background = match program() {
        Some(program) => base
            .child(
                shader()
                    .absolute()
                    .inset_0()
                    .source(program)
                    .uniform("a", move || colors()[0])
                    .uniform("b", move || colors()[1])
                    .uniform("c", move || colors()[2])
                    .uniform("intensity", 1.35_f32),
            )
            .into_element(),
        None => base
            .bg(move || AdaptiveColor::from(colors()[0]))
            .child(view().absolute().inset_0().bg(move || theme().background.opacity(0.4)))
            .into_element(),
    };
    view().flex_1().child(background).child(child)
}
