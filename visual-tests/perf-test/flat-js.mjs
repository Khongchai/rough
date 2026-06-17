// Shared pure-JS flat-buffer generators for the browser perf pages (line / polygon /
// path). Batch versions write ops into a preallocated Float64Array (stride 7) so the
// JS-flat baseline is measured fairly against the WASM batch APIs. Mirrors the Rust lib
// and src/fidelity-impls.ts. Path parsing reuses the real path-data-parser.

import { parsePath, absolutize, normalize } from '../../node_modules/path-data-parser/lib/index.js';

export const OP_STRIDE = 7;
export const OP_MOVE = 0;
export const OP_BCURVE = 1;
export const OP_LINE = 2;
export const STROKE_WIDTH = 1;

export class FlatRandom {
  constructor(seed) {
    this.seed = seed;
  }
  next() {
    this.seed = Math.imul(48271, this.seed);
    return (this.seed & 0x7fffffff) / 2147483648;
  }
}

// A cursor-based emitter over a preallocated buffer, with the _doubleLine / _bezierTo
// machinery shared by line / polygon / path.
function machine(buf, o, rng) {
  const state = { p: 0 };
  const offset = (min, max, rg) => o.roughness * rg * (rng.next() * (max - min) + min);
  const offsetOpt = (v, rg) => offset(-v, v, rg);

  function move(x, y) {
    const p = state.p;
    buf[p] = OP_MOVE;
    buf[p + 1] = x;
    buf[p + 2] = y;
    buf[p + 3] = 0;
    buf[p + 4] = 0;
    buf[p + 5] = 0;
    buf[p + 6] = 0;
    state.p += 7;
  }
  function bcurve(c1x, c1y, c2x, c2y, ex, ey) {
    const p = state.p;
    buf[p] = OP_BCURVE;
    buf[p + 1] = c1x;
    buf[p + 2] = c1y;
    buf[p + 3] = c2x;
    buf[p + 4] = c2y;
    buf[p + 5] = ex;
    buf[p + 6] = ey;
    state.p += 7;
  }

  function line(x1, y1, x2, y2, overlay) {
    const lengthSq = (x1 - x2) ** 2 + (y1 - y2) ** 2;
    const length = Math.sqrt(lengthSq);
    let rg;
    if (length < 200) rg = 1;
    else if (length > 500) rg = 0.4;
    else rg = -0.0016668 * length + 1.233334;

    let off = o.maxRandomnessOffset;
    if (off * off * 100 > lengthSq) off = length / 10;
    const halfOffset = off / 2;
    const divergePoint = 0.2 + rng.next() * 0.2;

    let midDispX = (o.bowing * o.maxRandomnessOffset * (y2 - y1)) / 200;
    let midDispY = (o.bowing * o.maxRandomnessOffset * (x1 - x2)) / 200;
    midDispX = offsetOpt(midDispX, rg);
    midDispY = offsetOpt(midDispY, rg);

    const pv = o.preserveVertices;
    const r = overlay ? halfOffset : off;

    move(x1 + (pv ? 0 : offsetOpt(r, rg)), y1 + (pv ? 0 : offsetOpt(r, rg)));
    bcurve(
      midDispX + x1 + (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + (y2 - y1) * divergePoint + offsetOpt(r, rg),
      midDispX + x1 + 2 * (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + 2 * (y2 - y1) * divergePoint + offsetOpt(r, rg),
      x2 + (pv ? 0 : offsetOpt(r, rg)),
      y2 + (pv ? 0 : offsetOpt(r, rg))
    );
  }

  function doubleLine(x1, y1, x2, y2) {
    line(x1, y1, x2, y2, false);
    if (!o.disableMultiStroke) line(x1, y1, x2, y2, true);
  }

  function bezierTo(x1, y1, x2, y2, x, y, cur) {
    const base = o.maxRandomnessOffset || 1;
    const ros = [base, base + 0.3];
    const iterations = o.disableMultiStroke ? 1 : 2;
    const pv = o.preserveVertices;
    for (let i = 0; i < iterations; i++) {
      if (i === 0) move(cur[0], cur[1]);
      else move(cur[0] + (pv ? 0 : offsetOpt(ros[0], 1)), cur[1] + (pv ? 0 : offsetOpt(ros[0], 1)));
      const fx = pv ? x : x + offsetOpt(ros[i], 1);
      const fy = pv ? y : y + offsetOpt(ros[i], 1);
      bcurve(x1 + offsetOpt(ros[i], 1), y1 + offsetOpt(ros[i], 1), x2 + offsetOpt(ros[i], 1), y2 + offsetOpt(ros[i], 1), fx, fy);
    }
  }

  return { state, doubleLine, bezierTo };
}

/** `coords`: flat [x1,y1,x2,y2,...]. Writes into `buf`, returns floats written. */
export function genLinesFlat(coords, buf, o, seed) {
  const m = machine(buf, o, new FlatRandom(seed));
  const n = coords.length / 4;
  for (let i = 0; i < n; i++) {
    const b = i * 4;
    m.doubleLine(coords[b], coords[b + 1], coords[b + 2], coords[b + 3]);
  }
  return m.state.p;
}

/** `points`: flat verts-vertex polygons end to end. Writes into `buf`, returns length. */
export function genPolygonsFlat(points, verts, buf, o, seed) {
  const m = machine(buf, o, new FlatRandom(seed));
  const n = points.length / (verts * 2);
  for (let pi = 0; pi < n; pi++) {
    const base = pi * verts * 2;
    if (verts > 2) {
      for (let v = 0; v < verts - 1; v++) {
        m.doubleLine(points[base + v * 2], points[base + v * 2 + 1], points[base + (v + 1) * 2], points[base + (v + 1) * 2 + 1]);
      }
      m.doubleLine(points[base + (verts - 1) * 2], points[base + (verts - 1) * 2 + 1], points[base], points[base + 1]);
    } else if (verts === 2) {
      m.doubleLine(points[base], points[base + 1], points[base + 2], points[base + 3]);
    }
  }
  return m.state.p;
}

/** One SVG path string generated `repeat` times. Writes into `buf`, returns length. */
export function genPathsFlat(d, repeat, buf, o, seed) {
  const m = machine(buf, o, new FlatRandom(seed));
  const pd = d.replace(/\n/g, ' ').replace(/(-\s)/g, '-');
  const segments = normalize(absolutize(parsePath(pd)));
  for (let r = 0; r < repeat; r++) {
    let first = [0, 0];
    let cur = [0, 0];
    for (const { key, data } of segments) {
      switch (key) {
        case 'M':
          cur = [data[0], data[1]];
          first = [data[0], data[1]];
          break;
        case 'L':
          m.doubleLine(cur[0], cur[1], data[0], data[1]);
          cur = [data[0], data[1]];
          break;
        case 'C':
          m.bezierTo(data[0], data[1], data[2], data[3], data[4], data[5], cur);
          cur = [data[4], data[5]];
          break;
        case 'Z':
          m.doubleLine(cur[0], cur[1], first[0], first[1]);
          cur = [first[0], first[1]];
          break;
      }
    }
  }
  return m.state.p;
}

/** Decode a flat stride-7 op buffer and stroke it (same as rough.js _drawToContext). */
export function drawOpBuffer(ctx, buf, width, height, len) {
  ctx.clearRect(0, 0, width, height);
  ctx.save();
  ctx.strokeStyle = 'white';
  ctx.lineWidth = STROKE_WIDTH;
  ctx.beginPath();
  const n = len === undefined ? buf.length : len;
  for (let i = 0; i < n; i += OP_STRIDE) {
    const op = buf[i];
    if (op === OP_MOVE) ctx.moveTo(buf[i + 1], buf[i + 2]);
    else if (op === OP_BCURVE) ctx.bezierCurveTo(buf[i + 1], buf[i + 2], buf[i + 3], buf[i + 4], buf[i + 5], buf[i + 6]);
    else if (op === OP_LINE) ctx.lineTo(buf[i + 1], buf[i + 2]);
  }
  ctx.stroke();
  ctx.restore();
}
