// Target-agnostic wrapper around the generated wasm-bindgen exports.
//
// This file has NO dependency on a specific wasm-pack target: it takes the generated
// bindings (web, nodejs, or bundler) and adapts them to the ergonomic `ShapeImpl` API.
// The web entry (index.ts) wires this up with `--target web` + async init; the test
// harness wires it up with the `--target nodejs` build.

export const OP_STRIDE = 7;
export const OP_MOVE = 0;
export const OP_BCURVE = 1;
export const OP_LINE = 2;

/** The subset of RoughJS options the rectangle + ellipse stroke paths read. */
export interface FlatOptions {
  roughness: number;
  maxRandomnessOffset: number;
  bowing: number;
  preserveVertices: boolean;
  disableMultiStroke: boolean;
  curveStepCount: number;
  curveFitting: number;
  curveTightness: number;
}

/** RoughJS's default values for the options above. */
export const DEFAULTS: FlatOptions = {
  roughness: 1,
  maxRandomnessOffset: 2,
  bowing: 1,
  preserveVertices: false,
  disableMultiStroke: false,
  curveStepCount: 9,
  curveFitting: 0.95,
  curveTightness: 0,
};

/**
 * All generators return a flat stride-7 Float64Array:
 *   [opcode, d0, d1, d2, d3, d4, d5, opcode, ...]
 * where opcode is OP_MOVE | OP_BCURVE | OP_LINE (move/line use d0..d1).
 *
 * IMPORTANT: the returned array is a zero-copy VIEW over WASM linear memory, valid only
 * until the next generate call (or any other WASM call), which reuses the underlying
 * buffer. Call `.slice()` to keep a stable copy.
 */
export interface ShapeImpl {
  name: string;
  /** Bounding-box shapes (rectangle, ellipse). x,y is top-left for rect, centre for ellipse. */
  generate(x: number, y: number, w: number, h: number, o: FlatOptions, seed: number): Float64Array;
}

export interface LineImpl {
  name: string;
  generate(x1: number, y1: number, x2: number, y2: number, o: FlatOptions, seed: number): Float64Array;
}

export interface PolygonImpl {
  name: string;
  /** `points` is a flat [x0,y0, x1,y1, ...] vertex buffer; the polygon is auto-closed. */
  generate(points: ArrayLike<number>, o: FlatOptions, seed: number): Float64Array;
}

export interface PathImpl {
  name: string;
  /** `d` is an SVG path data string. */
  generate(d: string, o: FlatOptions, seed: number): Float64Array;
}

export interface RoughShapes {
  rectangle: ShapeImpl;
  ellipse: ShapeImpl;
  line: LineImpl;
  polygon: PolygonImpl;
  path: PathImpl;
}

/** Structural type of the generated wasm-bindgen exports this wrapper relies on. */
export interface RoughWasmExports {
  generate_rectangles_view(
    rects: Float64Array,
    roughness: number,
    maxRandomnessOffset: number,
    bowing: number,
    preserveVertices: boolean,
    disableMultiStroke: boolean,
    seed: number
  ): Float64Array;
  generate_ellipses_view(
    ellipses: Float64Array,
    roughness: number,
    maxRandomnessOffset: number,
    bowing: number,
    preserveVertices: boolean,
    disableMultiStroke: boolean,
    curveStepCount: number,
    curveFitting: number,
    curveTightness: number,
    seed: number
  ): Float64Array;
  generate_lines_view(
    coords: Float64Array,
    roughness: number,
    maxRandomnessOffset: number,
    bowing: number,
    preserveVertices: boolean,
    disableMultiStroke: boolean,
    seed: number
  ): Float64Array;
  generate_polygons_view(
    points: Float64Array,
    verts: number,
    roughness: number,
    maxRandomnessOffset: number,
    bowing: number,
    preserveVertices: boolean,
    disableMultiStroke: boolean,
    seed: number
  ): Float64Array;
  generate_path_view(
    d: string,
    repeat: number,
    roughness: number,
    maxRandomnessOffset: number,
    bowing: number,
    preserveVertices: boolean,
    disableMultiStroke: boolean,
    seed: number
  ): Float64Array;
}

/** Adapt generated wasm bindings to the ergonomic ShapeImpl API. */
export function makeImpls(wasm: RoughWasmExports): RoughShapes {
  return {
    rectangle: {
      name: 'wasm-view',
      generate(x, y, w, h, o, seed) {
        return wasm.generate_rectangles_view(
          new Float64Array([x, y, w, h]),
          o.roughness,
          o.maxRandomnessOffset,
          o.bowing,
          o.preserveVertices,
          o.disableMultiStroke,
          seed
        );
      },
    },
    ellipse: {
      name: 'wasm-view',
      generate(x, y, w, h, o, seed) {
        return wasm.generate_ellipses_view(
          new Float64Array([x, y, w, h]),
          o.roughness,
          o.maxRandomnessOffset,
          o.bowing,
          o.preserveVertices,
          o.disableMultiStroke,
          o.curveStepCount,
          o.curveFitting,
          o.curveTightness,
          seed
        );
      },
    },
    line: {
      name: 'wasm-view',
      generate(x1, y1, x2, y2, o, seed) {
        return wasm.generate_lines_view(
          new Float64Array([x1, y1, x2, y2]),
          o.roughness,
          o.maxRandomnessOffset,
          o.bowing,
          o.preserveVertices,
          o.disableMultiStroke,
          seed
        );
      },
    },
    polygon: {
      name: 'wasm-view',
      generate(points, o, seed) {
        const pts = points instanceof Float64Array ? points : Float64Array.from(points);
        return wasm.generate_polygons_view(
          pts,
          pts.length / 2,
          o.roughness,
          o.maxRandomnessOffset,
          o.bowing,
          o.preserveVertices,
          o.disableMultiStroke,
          seed
        );
      },
    },
    path: {
      name: 'wasm-view',
      generate(d, o, seed) {
        return wasm.generate_path_view(
          d,
          1,
          o.roughness,
          o.maxRandomnessOffset,
          o.bowing,
          o.preserveVertices,
          o.disableMultiStroke,
          seed
        );
      },
    },
  };
}
