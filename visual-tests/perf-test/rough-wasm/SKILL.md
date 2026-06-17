---
name: migrate-rough-generator-cached-to-wasm
description: >
  Migrate a RoughGeneratorCached wrapper to use the rough-wasm-shapes package
  (Rust/WASM port of RoughJS stroke generation) for unfilled shapes, while
  falling back to the original roughjs generator for filled shapes. Use when
  asked to speed up rough.js shape generation or wire in rough-wasm-shapes.
---

# Migrate `RoughGeneratorCached` to rough-wasm-shapes

## Goal

`rough-wasm-shapes` is a Rust→WASM port of RoughJS's **stroke** generation. It is
much faster than `roughjs` at producing the sketchy geometry, but it is
**stroke-only** (no fill). Migrate `RoughGeneratorCached` so that:

- **Unfilled shapes** → generate geometry with `rough-wasm-shapes` (fast path).
- **Filled shapes** → keep using the original `roughjs` `RoughGenerator` (unchanged).

The split lives in `RoughGeneratorCached`, NOT in the package. The package stays
stroke-only and lean.

## The decision rule

```ts
const hasFill = (o: Options) =>
  !!o.fill && o.fill !== 'transparent' && o.fill !== 'none';
```

`hasFill(opts) === true` → rough.js path (returns a `Drawable`, as today).
`hasFill(opts) === false` → WASM path (returns a flat op buffer).

## What the WASM package returns (READ THIS FIRST)

`generate()` does NOT return a rough.js `Drawable`. It returns a **flat
`Float64Array`** of drawing ops, stride 7:

```
[ opcode, d0, d1, d2, d3, d4, d5,  opcode, d0, ...,  ... ]
```

- `opcode`: `0 = moveTo` (uses d0,d1), `1 = bezierCurveTo` (uses d0..d5), `2 = lineTo` (uses d0,d1).
- It carries **geometry only — no styling**. You apply `strokeStyle`/`lineWidth` yourself from `opts`.
- It is **stroke ops only** (one path). There are no fill ops and no opset-type headers.
- ⚠️ The returned array is a **zero-copy view over WASM memory**, valid only until the
  **next** `generate()` call. **Call `.slice()` immediately** to keep a stable copy
  (required before caching it).

### Decode + draw recipe

```ts
const OP_STRIDE = 7, OP_MOVE = 0, OP_BCURVE = 1, OP_LINE = 2;

function drawOps(ctx: CanvasRenderingContext2D, buf: Float64Array, dx: number, dy: number, strokeStyle: string, lineWidth: number) {
  ctx.save();
  ctx.strokeStyle = strokeStyle;
  ctx.lineWidth = lineWidth;
  ctx.beginPath();
  for (let i = 0; i < buf.length; i += OP_STRIDE) {
    const op = buf[i];
    if (op === OP_MOVE) ctx.moveTo(buf[i + 1] + dx, buf[i + 2] + dy);
    else if (op === OP_BCURVE) ctx.bezierCurveTo(buf[i + 1] + dx, buf[i + 2] + dy, buf[i + 3] + dx, buf[i + 4] + dy, buf[i + 5] + dx, buf[i + 6] + dy);
    else if (op === OP_LINE) ctx.lineTo(buf[i + 1] + dx, buf[i + 2] + dy);
  }
  ctx.stroke();
  ctx.restore();
}
```

`dx`/`dy` are the same translation offsets `RoughGeneratorCached` already returns —
see "Keep the cache + translate pattern" below.

## Package API

Install/link the package, build the WASM, then init once:

```bash
# in the rough-wasm-shapes package
npm run build:wasm        # wasm-pack --target web -> pkg/  (REQUIRED before use)
```

```ts
import { load, DEFAULTS, type FlatOptions } from 'rough-wasm-shapes';

const shapes = await load();   // init WASM once (memoized); call before generating
```

Generators (all return `Float64Array`; `seed` must be NON-ZERO — see below):

| Method | Signature |
|---|---|
| rectangle | `shapes.rectangle.generate(x, y, w, h, flatOpts, seed)` |
| ellipse | `shapes.ellipse.generate(cx, cy, w, h, flatOpts, seed)`  (x,y is CENTRE) |
| line | `shapes.line.generate(x1, y1, x2, y2, flatOpts, seed)` |
| polygon | `shapes.polygon.generate(flatPoints /* [x0,y0,x1,y1,...] */, flatOpts, seed)` (auto-closed) |
| path | `shapes.path.generate(d /* SVG path string */, flatOpts, seed)` |

## Options mapping: roughjs `Options` → `FlatOptions`

The package reads only the 8 geometry options below. Stroke color/width/dashes are
NOT options here — apply them at draw time. Always start from `DEFAULTS`:

```ts
function toFlat(o: Options): FlatOptions {
  return {
    ...DEFAULTS, // roughness:1, maxRandomnessOffset:2, bowing:1, preserveVertices:false,
                 // disableMultiStroke:false, curveStepCount:9, curveFitting:0.95, curveTightness:0
    roughness: o.roughness ?? DEFAULTS.roughness,
    maxRandomnessOffset: o.maxRandomnessOffset ?? DEFAULTS.maxRandomnessOffset,
    bowing: o.bowing ?? DEFAULTS.bowing,
    preserveVertices: !!o.preserveVertices,
    disableMultiStroke: !!o.disableMultiStroke,
    curveStepCount: o.curveStepCount ?? DEFAULTS.curveStepCount,
    curveFitting: o.curveFitting ?? DEFAULTS.curveFitting,
    curveTightness: o.curveTightness ?? DEFAULTS.curveTightness,
  };
}
```

### Seed (critical)

- WASM uses the seeded Park-Miller LCG only. **`seed` must be non-zero.** Seed `0`
  (or omitted) produces degenerate output — unlike roughjs, which falls back to
  `Math.random()` for seed 0.
- For caching + stable hit-testing you want determinism anyway, so pass a fixed
  non-zero seed. In the current code `ellipse` already pins `seed: 4` — keep that;
  give the other shapes a fixed seed too (e.g. read `opts.seed`, else use `1`).

## Keep the cache + translate pattern

`RoughGeneratorCached` generates each shape at a normalized origin and returns
`{ drawable, dx, dy }` so identical shapes share a cache entry regardless of
position. Preserve this exactly — just swap what `create()` produces:

- `rectangle`: generate at `(0,0,w,h)`, `dx=x, dy=y`.
- `ellipse`: generate at `(0,0,w,h)`, `dx=cx, dy=cy`.
- `line`: generate at `(0,0, x2-x1, y2-y1)`, `dx=x1, dy=y1`.
- `polygon`: generate with points made relative to `points[0]`, `dx=ax, dy=ay`.
- `path` / `roundedRectPath`: generated from the `d` string (no translation, `dx=dy=0`), cached by `d`+opts.

WASM geometry is translation-invariant for a fixed seed (the sketch offsets are
seed-driven, not position-driven), so this caching stays valid. `drawOps` applies
`dx/dy` at draw time. (You could also drop the cache for WASM shapes — regeneration
is cheap and deterministic — but keeping it is fine and minimally invasive.)

## Return type

Make the cached value a tagged union so the draw layer knows which engine produced it:

```ts
type CachedShape =
  | { kind: 'drawable'; drawable: Drawable; dx: number; dy: number } // filled -> rough.js (today)
  | { kind: 'ops'; ops: Float64Array; dx: number; dy: number };      // unfilled -> WASM buffer

// draw site:
if (s.kind === 'drawable') rc.draw(translate(s.drawable, s.dx, s.dy)); // existing behaviour
else drawOps(ctx, s.ops, s.dx, s.dy, opts.stroke ?? '#000', opts.strokeWidth ?? 1);
```

Cache the WASM result as `ops: view.slice()` (NOT the raw view).

## Coverage and gaps (do not assume parity)

Covered (stroke geometry, matches rough.js within ~1e-9):
- `rectangle`, `ellipse`, `line`, `polygon`, and SVG `path` (M/L/H/V/C/S/Q/T/Z + relative).
- `path` supports arcs (`A`), so `roundedRectPath`'s arc `d` works geometrically.

Gaps — these MUST route to the rough.js fallback (or stay on rough.js):
- **Fill of any kind** — the whole reason for the fallback. `hasFill(opts)` → rough.js.
- **`opts.simplification`** — the WASM path only implements the default (non-simplified)
  branch. If `simplification` is set, use rough.js.
- **Arc bit-exactness** — `A` arcs use trig (`tan`/`asin`); WASM (Rust libm) and rough.js
  (V8) differ by ULPs. Visually identical and WASM-vs-WASM is consistent (stable
  hit-testing), but it won't match a rough.js render pixel-for-pixel. Fine for rendering.

## Hit-testing note

A WASM shape is an op buffer, not a `Drawable` (no `options`, no rough.js geometry
object). If you hit-test against the generated geometry, do it against the decoded
buffer (the move/curve/line points, translated by dx/dy). If hit-testing on a given
shape needs the rough.js `Drawable`, keep that shape on the rough.js path.

## Verifying a port

This repo contains a fidelity harness proving the WASM output matches rough.js:
`src/{rectangle,ellipse,line,polygon,path}.test.ts` + `src/fidelity-impls.ts`
(run `npm test`; needs the Node build `npm run build:wasm-node`). Mirror its decode
(`flatten` of rough.js `Drawable.sets` vs the WASM buffer) if you need to assert
parity in the target project.

## Step-by-step

1. Add `rough-wasm-shapes` as a dependency; run `npm run build:wasm`; `await load()` at startup.
2. Add `hasFill`, `toFlat`, a fixed non-zero seed policy, and `drawOps`.
3. Change the cached value to the `CachedShape` union.
4. In each method's `create()`: if `hasFill(opts) || opts.simplification` → existing rough.js call (`kind:'drawable'`); else → `shapes.X.generate(...).slice()` (`kind:'ops'`), keeping the same origin-relative coords + `dx/dy`.
5. Update the draw site to branch on `kind` and apply `strokeStyle`/`lineWidth` from `opts` for the `ops` branch.
6. Verify a few shapes against rough.js output (see "Verifying a port").
