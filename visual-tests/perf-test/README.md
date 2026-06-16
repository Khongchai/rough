# perf-test

Benchmarks for RoughJS rectangle generation, plus a Rust/WASM port of the
stroke-only `rectangle` path to test whether Rust speeds up generation.

## Files

- `rectangle.html` — JS-only: generate + draw 6000 rectangles, averaged over 10 runs.
- `compare.html` — rectangle: JS rough.js vs JS flat-buffer vs Rust/WASM, same inputs.
- `ellipse.html` — ellipse: same five-way comparison as `compare.html`.
- `rough-wasm/` — the Rust crate (port of `src/renderer.ts` rectangle + ellipse paths).

## Build the WASM module

```bash
cd visual-tests/perf-test/rough-wasm
wasm-pack build --target web --release
```

This writes `rough-wasm/pkg/`, which `compare.html` imports.

## Run

Serve the repo root over HTTP (ES modules + WASM fetch need it), then open the page:

```bash
# from repo root
npm run build            # only if you changed src/ (rebuilds bundled/rough.esm.js)
python3 -m http.server 8000
# open http://localhost:8000/visual-tests/perf-test/compare.html
```

## What's measured

Only the **generation** phase. Drawing is `CanvasRenderingContext2D` work — the
browser's canvas backend — so WASM can't speed it up. `compare.html` reports:

- **JS rough.js gen** — `rc.generator.rectangle(...)` per rectangle.
- **WASM gen (full)** — includes returning the op buffer across the JS↔WASM boundary.
- **WASM gen (pure)** — the generation algorithm only (returns a checksum), isolating
  raw compute from marshalling cost.

Both sides use the same seeded Park-Miller LCG so the comparison is RNG-cost-fair.

## The port

`rough-wasm/src/lib.rs` mirrors `rectangle -> polygon -> linearPath -> _doubleLine
-> _line`. Ops are integer-tagged (`OpType`: Move=0/BCurveTo=1/LineTo=2;
`OpSetType`: Path=0/FillPath=1/FillSketch=2) and emitted into a flat `f64` buffer
(stride 7: `[opcode, d0..d5]`). Only the 6 option fields the stroke path actually
reads are modeled in `RectOptions`.
