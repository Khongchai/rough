// Test harness for the rectangle/ellipse fidelity tests.
//
// The WASM-facing API (ShapeImpl, FlatOptions, DEFAULTS, makeImpls) now lives in the
// reusable package at visual-tests/perf-test/rough-wasm (core.ts). Here we:
//   - feed the Node-target bindings into makeImpls to get the wasm-view impls,
//   - keep the rough.js reference (the oracle) and the pure-JS flat-buffer port, which
//     are test-only and don't belong in the shipped package.
//
// The Node-target WASM loads synchronously (unlike the browser `--target web` build).
// Rebuild it with:
//   cd visual-tests/perf-test/rough-wasm && npm run build:wasm-node

import { parsePath, absolutize, normalize } from 'path-data-parser';
import * as nodeWasm from '../visual-tests/perf-test/rough-wasm/pkg-node/rough_wasm.js';
import {
  makeImpls,
  DEFAULTS,
  OP_STRIDE,
  OP_MOVE,
  OP_BCURVE,
  OP_LINE,
  type FlatOptions,
  type ShapeImpl,
  type LineImpl,
  type PolygonImpl,
  type PathImpl,
  type RoughWasmExports,
} from '../visual-tests/perf-test/rough-wasm/core.js';
import { OpSet, Options } from './core.js';
import { Point } from './geometry.js';
import { RoughGenerator } from './generator.js';

// Re-export the package surface so the test files can import everything from here.
export { DEFAULTS, OP_STRIDE, OP_MOVE, OP_BCURVE, OP_LINE };
export type { FlatOptions, ShapeImpl, LineImpl, PolygonImpl, PathImpl };

// --- rough.js reference (the oracle) ---

const gen = new RoughGenerator();

function toOptions(o: FlatOptions, seed: number): Options {
  return {
    seed,
    roughness: o.roughness,
    maxRandomnessOffset: o.maxRandomnessOffset,
    bowing: o.bowing,
    preserveVertices: o.preserveVertices,
    disableMultiStroke: o.disableMultiStroke,
    curveStepCount: o.curveStepCount,
    curveFitting: o.curveFitting,
    curveTightness: o.curveTightness,
  };
}

/** Decode rough.js OpSets (stroke paths) into the flat stride-7 buffer. */
function flatten(sets: OpSet[]): Float64Array {
  const out: number[] = [];
  for (const set of sets) {
    for (const op of set.ops) {
      const code = op.op === 'move' ? OP_MOVE : op.op === 'bcurveTo' ? OP_BCURVE : OP_LINE;
      const d = op.data;
      out.push(code, d[0] ?? 0, d[1] ?? 0, d[2] ?? 0, d[3] ?? 0, d[4] ?? 0, d[5] ?? 0);
    }
  }
  return new Float64Array(out);
}

export function referenceRectangle(x: number, y: number, w: number, h: number, o: FlatOptions, seed: number): Float64Array {
  return flatten(gen.rectangle(x, y, w, h, toOptions(o, seed)).sets);
}

export function referenceEllipse(x: number, y: number, w: number, h: number, o: FlatOptions, seed: number): Float64Array {
  return flatten(gen.ellipse(x, y, w, h, toOptions(o, seed)).sets);
}

export function referenceLine(x1: number, y1: number, x2: number, y2: number, o: FlatOptions, seed: number): Float64Array {
  return flatten(gen.line(x1, y1, x2, y2, toOptions(o, seed)).sets);
}

export function referencePolygon(points: ArrayLike<number>, o: FlatOptions, seed: number): Float64Array {
  const pts: Point[] = [];
  for (let i = 0; i < points.length; i += 2) pts.push([points[i], points[i + 1]]);
  return flatten(gen.polygon(pts, toOptions(o, seed)).sets);
}

export function referencePath(d: string, o: FlatOptions, seed: number): Float64Array {
  return flatten(gen.path(d, toOptions(o, seed)).sets);
}

// --- WASM champion (zero-copy view). The package returns a transient view; the tests
// wrap it to snapshot via .slice() so held buffers survive later generate() calls. ---

const wasmShapes = makeImpls(nodeWasm as unknown as RoughWasmExports);

const sliced = (impl: ShapeImpl): ShapeImpl => ({
  name: impl.name,
  generate: (x, y, w, h, o, seed) => impl.generate(x, y, w, h, o, seed).slice(),
});

// --- Pure-JS flat-buffer port (test-only oracle-conforming second impl) ---

class FlatRandom {
  seed: number;
  constructor(seed: number) {
    this.seed = seed;
  }
  next(): number {
    this.seed = Math.imul(48271, this.seed);
    return (this.seed & 0x7fffffff) / 2147483648;
  }
}

function genRectangleFlatJS(x: number, y: number, w: number, h: number, o: FlatOptions, seed: number): Float64Array {
  const rng = new FlatRandom(seed);
  const out: number[] = [];

  const offset = (min: number, max: number, rg: number) => o.roughness * rg * (rng.next() * (max - min) + min);
  const offsetOpt = (v: number, rg: number) => offset(-v, v, rg);

  function line(x1: number, y1: number, x2: number, y2: number, overlay: boolean) {
    const lengthSq = (x1 - x2) ** 2 + (y1 - y2) ** 2;
    const length = Math.sqrt(lengthSq);
    let rg: number;
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

    out.push(OP_MOVE, x1 + (pv ? 0 : offsetOpt(r, rg)), y1 + (pv ? 0 : offsetOpt(r, rg)), 0, 0, 0, 0);
    out.push(
      OP_BCURVE,
      midDispX + x1 + (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + (y2 - y1) * divergePoint + offsetOpt(r, rg),
      midDispX + x1 + 2 * (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + 2 * (y2 - y1) * divergePoint + offsetOpt(r, rg),
      x2 + (pv ? 0 : offsetOpt(r, rg)),
      y2 + (pv ? 0 : offsetOpt(r, rg))
    );
  }

  function doubleLine(x1: number, y1: number, x2: number, y2: number) {
    line(x1, y1, x2, y2, false);
    if (!o.disableMultiStroke) line(x1, y1, x2, y2, true);
  }

  const x2 = x + w;
  const y2 = y + h;
  doubleLine(x, y, x2, y);
  doubleLine(x2, y, x2, y2);
  doubleLine(x2, y2, x, y2);
  doubleLine(x, y2, x, y); // close
  return new Float64Array(out);
}

function genEllipseFlatJS(x: number, y: number, w: number, h: number, o: FlatOptions, seed: number): Float64Array {
  const rng = new FlatRandom(seed);
  const out: number[] = [];

  const offset = (min: number, max: number) => o.roughness * (rng.next() * (max - min) + min);
  const offsetOpt = (v: number) => offset(-v, v);

  function ellipseParams(width: number, height: number) {
    const psq = Math.sqrt(Math.PI * 2 * Math.sqrt(((width / 2) ** 2 + (height / 2) ** 2) / 2));
    const stepCount = Math.ceil(Math.max(o.curveStepCount, (o.curveStepCount / Math.sqrt(200)) * psq));
    const increment = (Math.PI * 2) / stepCount;
    let rx = Math.abs(width / 2);
    let ry = Math.abs(height / 2);
    const cfr = 1 - o.curveFitting;
    rx += offsetOpt(rx * cfr);
    ry += offsetOpt(ry * cfr);
    return { increment, rx, ry };
  }

  function computeEllipsePoints(increment: number, cx: number, cy: number, rx: number, ry: number, off: number, overlap: number): number[][] {
    const all: number[][] = [];
    if (o.roughness === 0) {
      const inc = increment / 4;
      all.push([cx + rx * Math.cos(-inc), cy + ry * Math.sin(-inc)]);
      for (let a = 0; a <= Math.PI * 2; a += inc) all.push([cx + rx * Math.cos(a), cy + ry * Math.sin(a)]);
      all.push([cx + rx, cy]);
      all.push([cx + rx * Math.cos(inc), cy + ry * Math.sin(inc)]);
    } else {
      const radOffset = offsetOpt(0.5) - Math.PI / 2;
      all.push([
        offsetOpt(off) + cx + 0.9 * rx * Math.cos(radOffset - increment),
        offsetOpt(off) + cy + 0.9 * ry * Math.sin(radOffset - increment),
      ]);
      const endAngle = Math.PI * 2 + radOffset - 0.01;
      for (let a = radOffset; a < endAngle; a += increment) {
        all.push([offsetOpt(off) + cx + rx * Math.cos(a), offsetOpt(off) + cy + ry * Math.sin(a)]);
      }
      all.push([
        offsetOpt(off) + cx + rx * Math.cos(radOffset + Math.PI * 2 + overlap * 0.5),
        offsetOpt(off) + cy + ry * Math.sin(radOffset + Math.PI * 2 + overlap * 0.5),
      ]);
      all.push([
        offsetOpt(off) + cx + 0.98 * rx * Math.cos(radOffset + overlap),
        offsetOpt(off) + cy + 0.98 * ry * Math.sin(radOffset + overlap),
      ]);
      all.push([
        offsetOpt(off) + cx + 0.9 * rx * Math.cos(radOffset + overlap * 0.5),
        offsetOpt(off) + cy + 0.9 * ry * Math.sin(radOffset + overlap * 0.5),
      ]);
    }
    return all;
  }

  function curve(points: number[][]) {
    const len = points.length;
    if (len > 3) {
      const s = 1 - o.curveTightness;
      out.push(OP_MOVE, points[1][0], points[1][1], 0, 0, 0, 0);
      for (let i = 1; i + 2 < len; i++) {
        out.push(
          OP_BCURVE,
          points[i][0] + (s * points[i + 1][0] - s * points[i - 1][0]) / 6,
          points[i][1] + (s * points[i + 1][1] - s * points[i - 1][1]) / 6,
          points[i + 1][0] + (s * points[i][0] - s * points[i + 2][0]) / 6,
          points[i + 1][1] + (s * points[i][1] - s * points[i + 2][1]) / 6,
          points[i + 1][0],
          points[i + 1][1]
        );
      }
    } else if (len === 3) {
      out.push(OP_MOVE, points[1][0], points[1][1], 0, 0, 0, 0);
      out.push(OP_BCURVE, points[1][0], points[1][1], points[2][0], points[2][1], points[2][0], points[2][1]);
    }
  }

  const params = ellipseParams(w, h);
  const inner = offset(0.4, 1);
  const overlap = params.increment * offset(0.1, inner);
  curve(computeEllipsePoints(params.increment, x, y, params.rx, params.ry, 1, overlap));
  if (!o.disableMultiStroke && o.roughness !== 0) {
    curve(computeEllipsePoints(params.increment, x, y, params.rx, params.ry, 1.5, 0));
  }
  return new Float64Array(out);
}

const jsRect: ShapeImpl = { name: 'js-flat', generate: genRectangleFlatJS };
const jsEllipse: ShapeImpl = { name: 'js-flat', generate: genEllipseFlatJS };

// --- Shared flat-JS machine for the _doubleLine / _bezierTo based shapes ---

interface LineMachine {
  offsetOpt(v: number, rg: number): number;
  doubleLine(x1: number, y1: number, x2: number, y2: number): void;
  bezierTo(x1: number, y1: number, x2: number, y2: number, x: number, y: number, current: [number, number]): void;
}

function lineMachine(o: FlatOptions, rng: FlatRandom, out: number[]): LineMachine {
  const offset = (min: number, max: number, rg: number) => o.roughness * rg * (rng.next() * (max - min) + min);
  const offsetOpt = (v: number, rg: number) => offset(-v, v, rg);

  function line(x1: number, y1: number, x2: number, y2: number, overlay: boolean) {
    const lengthSq = (x1 - x2) ** 2 + (y1 - y2) ** 2;
    const length = Math.sqrt(lengthSq);
    let rg: number;
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

    out.push(OP_MOVE, x1 + (pv ? 0 : offsetOpt(r, rg)), y1 + (pv ? 0 : offsetOpt(r, rg)), 0, 0, 0, 0);
    out.push(
      OP_BCURVE,
      midDispX + x1 + (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + (y2 - y1) * divergePoint + offsetOpt(r, rg),
      midDispX + x1 + 2 * (x2 - x1) * divergePoint + offsetOpt(r, rg),
      midDispY + y1 + 2 * (y2 - y1) * divergePoint + offsetOpt(r, rg),
      x2 + (pv ? 0 : offsetOpt(r, rg)),
      y2 + (pv ? 0 : offsetOpt(r, rg))
    );
  }

  function doubleLine(x1: number, y1: number, x2: number, y2: number) {
    line(x1, y1, x2, y2, false);
    if (!o.disableMultiStroke) line(x1, y1, x2, y2, true);
  }

  function bezierTo(x1: number, y1: number, x2: number, y2: number, x: number, y: number, current: [number, number]) {
    const base = o.maxRandomnessOffset || 1;
    const ros = [base, base + 0.3];
    const iterations = o.disableMultiStroke ? 1 : 2;
    const pv = o.preserveVertices;
    for (let i = 0; i < iterations; i++) {
      if (i === 0) {
        out.push(OP_MOVE, current[0], current[1], 0, 0, 0, 0);
      } else {
        out.push(OP_MOVE, current[0] + (pv ? 0 : offsetOpt(ros[0], 1)), current[1] + (pv ? 0 : offsetOpt(ros[0], 1)), 0, 0, 0, 0);
      }
      const fx = pv ? x : x + offsetOpt(ros[i], 1);
      const fy = pv ? y : y + offsetOpt(ros[i], 1);
      out.push(
        OP_BCURVE,
        x1 + offsetOpt(ros[i], 1),
        y1 + offsetOpt(ros[i], 1),
        x2 + offsetOpt(ros[i], 1),
        y2 + offsetOpt(ros[i], 1),
        fx,
        fy
      );
    }
  }

  return { offsetOpt, doubleLine, bezierTo };
}

function genLineFlatJS(x1: number, y1: number, x2: number, y2: number, o: FlatOptions, seed: number): Float64Array {
  const out: number[] = [];
  lineMachine(o, new FlatRandom(seed), out).doubleLine(x1, y1, x2, y2);
  return new Float64Array(out);
}

function genPolygonFlatJS(points: ArrayLike<number>, o: FlatOptions, seed: number): Float64Array {
  const out: number[] = [];
  const m = lineMachine(o, new FlatRandom(seed), out);
  const pts: [number, number][] = [];
  for (let i = 0; i < points.length; i += 2) pts.push([points[i], points[i + 1]]);
  const len = pts.length;
  // linearPath(points, close=true)
  if (len > 2) {
    for (let i = 0; i < len - 1; i++) m.doubleLine(pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1]);
    m.doubleLine(pts[len - 1][0], pts[len - 1][1], pts[0][0], pts[0][1]);
  } else if (len === 2) {
    m.doubleLine(pts[0][0], pts[0][1], pts[1][0], pts[1][1]);
  }
  return new Float64Array(out);
}

function genPathFlatJS(d: string, o: FlatOptions, seed: number): Float64Array {
  const out: number[] = [];
  if (!d) return new Float64Array(out);
  // generator.path preprocessing (the third JS replace is a no-op bug, so omitted).
  const pd = d.replace(/\n/g, ' ').replace(/(-\s)/g, '-');
  const m = lineMachine(o, new FlatRandom(seed), out);
  const segments = normalize(absolutize(parsePath(pd)));
  let first: [number, number] = [0, 0];
  let current: [number, number] = [0, 0];
  for (const { key, data } of segments) {
    switch (key) {
      case 'M':
        current = [data[0], data[1]];
        first = [data[0], data[1]];
        break;
      case 'L':
        m.doubleLine(current[0], current[1], data[0], data[1]);
        current = [data[0], data[1]];
        break;
      case 'C':
        m.bezierTo(data[0], data[1], data[2], data[3], data[4], data[5], current);
        current = [data[4], data[5]];
        break;
      case 'Z':
        m.doubleLine(current[0], current[1], first[0], first[1]);
        current = [first[0], first[1]];
        break;
    }
  }
  return new Float64Array(out);
}

const jsLine: LineImpl = { name: 'js-flat', generate: genLineFlatJS };
const jsPolygon: PolygonImpl = { name: 'js-flat', generate: genPolygonFlatJS };
const jsPath: PathImpl = { name: 'js-flat', generate: genPathFlatJS };

const slicedLine = (impl: LineImpl): LineImpl => ({
  name: impl.name,
  generate: (x1, y1, x2, y2, o, seed) => impl.generate(x1, y1, x2, y2, o, seed).slice(),
});
const slicedPolygon = (impl: PolygonImpl): PolygonImpl => ({
  name: impl.name,
  generate: (points, o, seed) => impl.generate(points, o, seed).slice(),
});
const slicedPath = (impl: PathImpl): PathImpl => ({
  name: impl.name,
  generate: (d, o, seed) => impl.generate(d, o, seed).slice(),
});

// The candidates under test. wasm-view first ("the champion").
export const rectangleImpls: ShapeImpl[] = [sliced(wasmShapes.rectangle), jsRect];
export const ellipseImpls: ShapeImpl[] = [sliced(wasmShapes.ellipse), jsEllipse];
export const lineImpls: LineImpl[] = [slicedLine(wasmShapes.line), jsLine];
export const polygonImpls: PolygonImpl[] = [slicedPolygon(wasmShapes.polygon), jsPolygon];
export const pathImpls: PathImpl[] = [slicedPath(wasmShapes.path), jsPath];
