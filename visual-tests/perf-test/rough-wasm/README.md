# rough-wasm-shapes

Rust/WASM port of RoughJS's rectangle + ellipse stroke generators, wrapped in an
ergonomic `ShapeImpl` API. Designed to be linked as an internal package in a turbo
monorepo (ships raw TS; the consumer's bundler transpiles it).

## Build the WASM bindings

The TS wrappers import the generated wasm-bindgen output, which is **not committed**
(see `.gitignore`). Generate it first:

```bash
npm run build:wasm        # --target web   -> pkg/      (browser, used by index.ts)
npm run build:wasm-node   # --target nodejs -> pkg-node/ (Node, used by the tests)
```

## Use it (browser, `--target web`)

`load()` initializes the WASM module once, then returns the generators.

```ts
import { load, DEFAULTS } from 'rough-wasm-shapes';

const shapes = await load();
// flat stride-7 op buffer [opcode, d0..d5, ...]; opcode 0=move 1=bcurveTo 2=lineTo
const rect = shapes.rectangle.generate(10, 10, 80, 80, DEFAULTS, /* seed */ 1).slice();
const ell = shapes.ellipse.generate(200, 200, 120, 120, DEFAULTS, 1).slice();
const ln = shapes.line.generate(10, 10, 90, 90, DEFAULTS, 1).slice();
const poly = shapes.polygon.generate([50, 10, 90, 90, 10, 90], DEFAULTS, 1).slice();
const pth = shapes.path.generate('M10 10 L90 10 L50 90 Z', DEFAULTS, 1).slice();
```

⚠️ `generate()` returns a **zero-copy view over WASM memory**, valid only until the next
`generate()` call. Call `.slice()` to keep a stable copy (as above).

## Target-agnostic core

`core.ts` has no target dependency: `makeImpls(wasmExports)` adapts any wasm-bindgen
build (web / nodejs / bundler) to the `ShapeImpl` API. `index.ts` is just the web
wiring; the test harness feeds the Node build into the same `makeImpls`.

## API

- `shapes.rectangle` / `shapes.ellipse`: `generate(x, y, w, h, opts, seed) => Float64Array`.
- `shapes.line`: `generate(x1, y1, x2, y2, opts, seed)`.
- `shapes.polygon`: `generate(points /* flat [x0,y0,...] */, opts, seed)` (auto-closed).
- `shapes.path`: `generate(d /* SVG path string */, opts, seed)`.
- `FlatOptions` / `DEFAULTS` — the option subset the stroke paths read.
- `OP_STRIDE`, `OP_MOVE`, `OP_BCURVE`, `OP_LINE` — buffer encoding constants.
- `seed` must be non-zero (seed 0 falls back to non-deterministic RNG in rough.js).
