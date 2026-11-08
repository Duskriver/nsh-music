// The ambient shader: soft colored light that drifts, tinted by the track. It
// shows NSH drawing a live WGSL shader behind native views. Falls back to a
// quiet gradient where shaders are unavailable.

import type { JSX } from "nsh";
import { Shader, View } from "nsh";

const AURORA = /* wgsl */ `
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
}`;

/** HSL (h in 0..1) to a `#rrggbb` string. */
function hsl(h: number, s: number, l: number): string {
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    const v = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(255 * v).toString(16).padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/** Three colors derived from a track URI, so each song has its own light. */
export function colorsFor(uri: string): [string, string, string] {
  if (!uri) return ["#1ed760", "#3b82f6", "#a855f7"];
  let hash = 0;
  for (let i = 0; i < uri.length; i++) hash = (hash * 31 + uri.charCodeAt(i)) >>> 0;
  const base = (hash % 360) / 360;
  return [hsl(base, 0.62, 0.5), hsl((base + 0.12) % 1, 0.62, 0.42), hsl((base + 0.6) % 1, 0.62, 0.38)];
}

/** The aurora, tinted by `colors`, under `children`. */
export function Ambient(props: { colors: () => [string, string, string]; children?: JSX.Element }) {
  return (
    <View class="flex-1">
      <Shader
        source={AURORA}
        uniforms={{ a: props.colors()[0], b: props.colors()[1], c: props.colors()[2], intensity: 1.35 }}
        style={{ position: "absolute", top: 0, left: 0, right: 0, bottom: 0 }}
      />
      {props.children}
    </View>
  );
}
