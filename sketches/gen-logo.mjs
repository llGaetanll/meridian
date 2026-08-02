// Generates the Meridian globe mark as static SVG.
//
// Every circle on a sphere projects to an ellipse under orthographic
// projection, so each meridian / parallel comes out as a handful of exact
// cubic Beziers instead of a sampled polyline.
//
//   node sketches/gen-logo.mjs

import { writeFileSync } from 'node:fs';

const OPTS = {
  spin: 317,
  tilt: 45,
  meridians: 3,
  parallels: 3,
  stroke: 14,
  accentStroke: 14,
};

const R = 86, CX = 100, CY = 100;
const rad = d => (d * Math.PI) / 180;
const fmt = n => +(Math.round(n * 1000) / 1000).toFixed(3);

// View transform: spin about the polar axis, then tilt the pole toward the
// viewer. Returns [X, Y, Z]; Z > 0 is the near hemisphere.
function view(g, tiltDeg, spinDeg) {
  const p0 = rad(tiltDeg), l0 = rad(spinDeg);
  const [gx, gy, gz] = g;
  return [
    -Math.sin(l0) * gx + Math.cos(l0) * gy,
    -Math.sin(p0) * Math.cos(l0) * gx - Math.sin(p0) * Math.sin(l0) * gy + Math.cos(p0) * gz,
    Math.cos(p0) * Math.cos(l0) * gx + Math.cos(p0) * Math.sin(l0) * gy + Math.sin(p0) * gz,
  ];
}

// A circle on the sphere: g(t) = n + a cos t + b sin t.
const meridian = lonDeg => {
  const l = rad(lonDeg);
  return { n: [0, 0, 0], a: [Math.cos(l), Math.sin(l), 0], b: [0, 0, 1] };
};
const parallel = latDeg => {
  const p = rad(latDeg), c = Math.cos(p);
  return { n: [0, 0, Math.sin(p)], a: [c, 0, 0], b: [0, c, 0] };
};

// Project the circle to screen space: point(t) = C + A cos t + B sin t,
// with the depth coefficients kept for the horizon solve.
function projectCircle({ n, a, b }, tilt, spin) {
  const [Xn, Yn, Zn] = view(n, tilt, spin);
  const [Xa, Ya, Za] = view(a, tilt, spin);
  const [Xb, Yb, Zb] = view(b, tilt, spin);
  return {
    C: [CX + R * Xn, CY - R * Yn],
    A: [R * Xa, -R * Ya],
    B: [R * Xb, -R * Yb],
    Z: [Zn, Za, Zb],
  };
}

// Zn + Za cos t + Zb sin t > 0  <=>  t within +-theta of psi.
function visibleRange([Zn, Za, Zb]) {
  const amp = Math.hypot(Za, Zb);
  if (amp <= Math.abs(Zn)) return Zn > 0 ? [0, 2 * Math.PI] : null;
  const psi = Math.atan2(Zb, Za);
  const theta = Math.acos(-Zn / amp);
  return [psi - theta, psi + theta];
}

const at = (e, t) => [
  e.C[0] + e.A[0] * Math.cos(t) + e.B[0] * Math.sin(t),
  e.C[1] + e.A[1] * Math.cos(t) + e.B[1] * Math.sin(t),
];
const deriv = (e, t) => [
  -e.A[0] * Math.sin(t) + e.B[0] * Math.cos(t),
  -e.A[1] * Math.sin(t) + e.B[1] * Math.cos(t),
];

// Bezier arc approximation, exact for circles and affine-covariant, so it is
// just as good for their elliptical shadows. Chunks are capped at 90 degrees.
function arcPath(e, t0, t1) {
  const span = t1 - t0;
  const chunks = Math.max(1, Math.ceil(Math.abs(span) / (Math.PI / 2)));
  const step = span / chunks;
  const k = (4 / 3) * Math.tan(step / 4);

  let d = `M ${at(e, t0).map(fmt).join(',')}`;
  for (let i = 0; i < chunks; i++) {
    const ta = t0 + i * step, tb = ta + step;
    const pa = at(e, ta), pb = at(e, tb);
    const da = deriv(e, ta), db = deriv(e, tb);
    const c1 = [pa[0] + k * da[0], pa[1] + k * da[1]];
    const c2 = [pb[0] - k * db[0], pb[1] - k * db[1]];
    d += ` C ${[...c1, ...c2, ...pb].map(fmt).join(',')}`;
  }
  return d;
}

function visiblePath(circle, tilt, spin) {
  const e = projectCircle(circle, tilt, spin);
  const range = visibleRange(e.Z);
  return range ? arcPath(e, range[0], range[1]) : null;
}

function grid(o, spin = o.spin) {
  const base = [], accent = [];
  for (let i = 0; i < o.meridians; i++) {
    const lon = (i * 180) / o.meridians;
    for (const l of [lon, lon + 180]) {
      const d = visiblePath(meridian(l), o.tilt, spin);
      if (d) (i === 0 ? accent : base).push(d);
    }
  }
  const parallels = [];
  for (let i = 1; i <= o.parallels; i++) {
    const d = visiblePath(parallel(-90 + (180 * i) / (o.parallels + 1)), o.tilt, spin);
    if (d) parallels.push(d);
  }
  return { base, accent, parallels };
}

const paths = ds => ds.map(d => `<path d="${d}"/>`).join('');
const clipR = fmt(R + OPTS.stroke / 2);

// ---- static mark ----------------------------------------------------------

function mark(o) {
  const { base, accent, parallels } = grid(o);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200"
     fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">
  <defs>
    <clipPath id="meridian-globe-clip">
      <circle cx="${CX}" cy="${CY}" r="${clipR}"/>
    </clipPath>
  </defs>
  <g clip-path="url(#meridian-globe-clip)">
    <g stroke-width="${o.stroke}">${paths([...base, ...parallels])}</g>
    <g stroke-width="${o.accentStroke}">${paths(accent)}</g>
  </g>
  <circle cx="${CX}" cy="${CY}" r="${R}" stroke-width="${o.stroke}"/>
</svg>
`;
}

// ---- spinning loader ------------------------------------------------------
// The silhouette and parallels never move, so only the meridians are keyframed.
// Each frame is a group revealed for one slice of the cycle via a negative
// animation delay.

function loader(o, frames = 48, seconds = 2.4) {
  const slice = 100 / frames;
  const groups = [];
  for (let f = 0; f < frames; f++) {
    const { base, accent } = grid(o, o.spin + (360 * f) / frames);
    groups.push(
      `<g class="f" style="animation-delay:${fmt(-(seconds * f) / frames)}s">` +
        `<g stroke-width="${o.stroke}">${paths(base)}</g>` +
        `<g stroke-width="${o.accentStroke}">${paths(accent)}</g>` +
      `</g>`,
    );
  }
  const { parallels } = grid(o);

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200"
     fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">
  <style>
    .f { opacity: 0; animation: meridian-spin ${seconds}s steps(1, end) infinite; }
    @keyframes meridian-spin {
      0%, ${fmt(slice)}% { opacity: 1; }
      ${fmt(slice + 0.001)}%, 100% { opacity: 0; }
    }
    @media (prefers-reduced-motion: reduce) {
      .f { animation: none; }
      .f:first-of-type { opacity: 1; }
    }
  </style>
  <defs>
    <clipPath id="meridian-loader-clip">
      <circle cx="${CX}" cy="${CY}" r="${clipR}"/>
    </clipPath>
  </defs>
  <g clip-path="url(#meridian-loader-clip)">
    <g stroke-width="${o.stroke}">${paths(parallels)}</g>
    ${groups.join('\n    ')}
  </g>
  <circle cx="${CX}" cy="${CY}" r="${R}" stroke-width="${o.stroke}"/>
</svg>
`;
}

// ---- check ----------------------------------------------------------------
// Compare the Bezier output against a direct point-by-point projection.

function selfCheck() {
  let worst = 0;
  for (const circle of [meridian(0), meridian(57), parallel(-45), parallel(30)]) {
    const e = projectCircle(circle, OPTS.tilt, OPTS.spin);
    const [t0, t1] = visibleRange(e.Z);
    const chunks = Math.max(1, Math.ceil((t1 - t0) / (Math.PI / 2)));
    const step = (t1 - t0) / chunks;
    const k = (4 / 3) * Math.tan(step / 4);
    for (let i = 0; i < chunks; i++) {
      const ta = t0 + i * step, tb = ta + step;
      const pa = at(e, ta), pb = at(e, tb), da = deriv(e, ta), db = deriv(e, tb);
      const c1 = [pa[0] + k * da[0], pa[1] + k * da[1]];
      const c2 = [pb[0] - k * db[0], pb[1] - k * db[1]];
      // cubic at s = 0.5 vs the true point at the mid parameter
      const mid = [0, 1].map(j => (pa[j] + 3 * c1[j] + 3 * c2[j] + pb[j]) / 8);
      const truth = at(e, (ta + tb) / 2);
      worst = Math.max(worst, Math.hypot(mid[0] - truth[0], mid[1] - truth[1]));
    }
  }
  return worst;
}

writeFileSync(new URL('./meridian-mark.svg', import.meta.url), mark(OPTS));
writeFileSync(new URL('./meridian-loader.svg', import.meta.url), loader(OPTS));
console.log(`wrote meridian-mark.svg, meridian-loader.svg`);
console.log(`max Bezier deviation: ${selfCheck().toFixed(4)} units (viewBox is 200)`);
