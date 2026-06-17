// Browser entry point (wasm-pack `--target web`). Call `load()` once to initialize the
// WASM module, then use the returned `rectangle` / `ellipse` generators.
//
//   import { load, DEFAULTS } from 'rough-wasm-shapes';
//   const shapes = await load();
//   const ops = shapes.rectangle.generate(10, 10, 80, 80, DEFAULTS, 1).slice();
//
// Requires the generated bindings: run `npm run build:wasm` (wasm-pack --target web).

import init, { generate_rectangles_view, generate_ellipses_view } from './pkg/rough_wasm.js';
import { makeImpls, RoughShapes } from './core.js';

export * from './core.js';

let cached: RoughShapes | null = null;

/**
 * Initialize the WASM module (once, memoized) and return the shape generators.
 * @param moduleOrPath optional override forwarded to wasm-bindgen init (URL or bytes).
 */
export async function load(moduleOrPath?: Parameters<typeof init>[0]): Promise<RoughShapes> {
  if (!cached) {
    await init(moduleOrPath);
    cached = makeImpls({ generate_rectangles_view, generate_ellipses_view });
  }
  return cached;
}
