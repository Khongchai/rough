//! Minimal Rust/WASM port of RoughJS's `generator.rectangle` (stroke-only path).
//!
//! Faithful to src/renderer.ts: rectangle -> polygon -> linearPath -> _doubleLine
//! -> _line, driven by the Park-Miller LCG from src/math.ts. Only the 6 option
//! fields the stroke path actually reads are modeled (see `RectOptions`).

use wasm_bindgen::prelude::*;

// --- Integer-encoded op tags (mirror core.ts OpType / OpSetType) ---

#[repr(u8)]
#[allow(dead_code)]
pub enum OpType {
    Move = 0,
    BCurveTo = 1,
    LineTo = 2,
}

#[repr(u8)]
#[allow(dead_code)]
pub enum OpSetType {
    Path = 0,
    FillPath = 1,
    FillSketch = 2,
}

/// Each op is emitted into a flat f64 buffer with a fixed stride of 7:
/// [opcode, d0, d1, d2, d3, d4, d5]. Move/LineTo use d0..d1; BCurveTo uses all 6.
const OP_STRIDE: usize = 7;

// --- The only options the stroke-only rectangle path reads ---

#[derive(Clone, Copy)]
struct RectOptions {
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
}

// --- Park-Miller / MINSTD PRNG, exactly as src/math.ts ---

struct Random {
    seed: i32,
}

impl Random {
    #[inline]
    fn next(&mut self) -> f64 {
        // JS: (2**31 - 1) & (this.seed = Math.imul(48271, this.seed))) / 2**31
        // Math.imul == wrapping 32-bit signed multiply. seed=0 in JS falls back to
        // Math.random(); we require a non-zero seed for deterministic output.
        self.seed = 48271i32.wrapping_mul(self.seed);
        ((self.seed & 0x7FFF_FFFF) as f64) / 2_147_483_648.0
    }
}

// --- Offset helpers (renderer.ts _offset / _offsetOpt) ---

#[inline]
fn offset(min: f64, max: f64, rng: &mut Random, o: &RectOptions, roughness_gain: f64) -> f64 {
    o.roughness * roughness_gain * ((rng.next() * (max - min)) + min)
}

#[inline]
fn offset_opt(x: f64, rng: &mut Random, o: &RectOptions, roughness_gain: f64) -> f64 {
    offset(-x, x, rng, o, roughness_gain)
}

/// Port of renderer.ts `_line`. Appends a `move` + a `bcurveTo` op into `out`.
fn line(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rng: &mut Random,
    o: &RectOptions,
    do_move: bool,
    overlay: bool,
    out: &mut Vec<f64>,
) {
    let length_sq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
    let length = length_sq.sqrt();
    let roughness_gain = if length < 200.0 {
        1.0
    } else if length > 500.0 {
        0.4
    } else {
        -0.0016668 * length + 1.233334
    };

    let mut off = o.max_randomness_offset;
    if (off * off * 100.0) > length_sq {
        off = length / 10.0;
    }
    let half_offset = off / 2.0;
    let diverge_point = 0.2 + rng.next() * 0.2;

    let mut mid_disp_x = o.bowing * o.max_randomness_offset * (y2 - y1) / 200.0;
    let mut mid_disp_y = o.bowing * o.max_randomness_offset * (x1 - x2) / 200.0;
    mid_disp_x = offset_opt(mid_disp_x, rng, o, roughness_gain);
    mid_disp_y = offset_opt(mid_disp_y, rng, o, roughness_gain);

    let pv = o.preserve_vertices;

    // move op
    if do_move {
        let (mx, my) = if overlay {
            (
                x1 + if pv { 0.0 } else { offset_opt(half_offset, rng, o, roughness_gain) },
                y1 + if pv { 0.0 } else { offset_opt(half_offset, rng, o, roughness_gain) },
            )
        } else {
            (
                x1 + if pv { 0.0 } else { offset_opt(off, rng, o, roughness_gain) },
                y1 + if pv { 0.0 } else { offset_opt(off, rng, o, roughness_gain) },
            )
        };
        out.extend_from_slice(&[OpType::Move as u8 as f64, mx, my, 0.0, 0.0, 0.0, 0.0]);
    }

    // bcurveTo op. `rh`/`rf` advance the rng on each call, matching JS call order.
    let rh = |rng: &mut Random| offset_opt(half_offset, rng, o, roughness_gain);
    let rf = |rng: &mut Random| offset_opt(off, rng, o, roughness_gain);

    let (c1x, c1y, c2x, c2y, ex, ey) = if overlay {
        let a = mid_disp_x + x1 + (x2 - x1) * diverge_point + rh(rng);
        let b = mid_disp_y + y1 + (y2 - y1) * diverge_point + rh(rng);
        let c = mid_disp_x + x1 + 2.0 * (x2 - x1) * diverge_point + rh(rng);
        let d = mid_disp_y + y1 + 2.0 * (y2 - y1) * diverge_point + rh(rng);
        let e = x2 + if pv { 0.0 } else { rh(rng) };
        let f = y2 + if pv { 0.0 } else { rh(rng) };
        (a, b, c, d, e, f)
    } else {
        let a = mid_disp_x + x1 + (x2 - x1) * diverge_point + rf(rng);
        let b = mid_disp_y + y1 + (y2 - y1) * diverge_point + rf(rng);
        let c = mid_disp_x + x1 + 2.0 * (x2 - x1) * diverge_point + rf(rng);
        let d = mid_disp_y + y1 + 2.0 * (y2 - y1) * diverge_point + rf(rng);
        let e = x2 + if pv { 0.0 } else { rf(rng) };
        let f = y2 + if pv { 0.0 } else { rf(rng) };
        (a, b, c, d, e, f)
    };
    out.extend_from_slice(&[OpType::BCurveTo as u8 as f64, c1x, c1y, c2x, c2y, ex, ey]);
}

/// Port of renderer.ts `_doubleLine` (filling = false).
fn double_line(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rng: &mut Random,
    o: &RectOptions,
    out: &mut Vec<f64>,
) {
    line(x1, y1, x2, y2, rng, o, true, false, out);
    if !o.disable_multi_stroke {
        line(x1, y1, x2, y2, rng, o, true, true, out);
    }
}

/// One rectangle outline (the closed 4-point linearPath). Appends ops to `out`.
fn rectangle_into(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rng: &mut Random,
    o: &RectOptions,
    out: &mut Vec<f64>,
) {
    let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    for i in 0..3 {
        double_line(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, rng, o, out);
    }
    // close
    double_line(pts[3].0, pts[3].1, pts[0].0, pts[0].1, rng, o, out);
}

fn make_opts(
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
) -> RectOptions {
    RectOptions {
        roughness,
        max_randomness_offset,
        bowing,
        preserve_vertices,
        disable_multi_stroke,
    }
}

/// Fill `out` with the op buffer for a batch of rectangles. `rects` is a flat
/// [x,y,w,h, ...] buffer. One shared RNG advances across all rects, mirroring how
/// rough.js reuses the randomizer on the shared options object.
fn fill_rectangles(out: &mut Vec<f64>, rects: &[f64], o: &RectOptions, seed: i32) {
    out.clear();
    let mut rng = Random { seed };
    let count = rects.len() / 4;
    // 4 doubleLines * 2 lines * 2 ops = 16 ops/rect when multi-stroke is on.
    let ops_per_rect = if o.disable_multi_stroke { 8 } else { 16 };
    out.reserve(count * ops_per_rect * OP_STRIDE);
    for i in 0..count {
        let b = i * 4;
        rectangle_into(rects[b], rects[b + 1], rects[b + 2], rects[b + 3], &mut rng, o, out);
    }
}

/// Copy-based API: returns a fresh `Vec<f64>`, which wasm-bindgen copies into a new
/// JS Float64Array across the boundary (~5.4 MB for 6000 rects). Kept for comparison.
#[wasm_bindgen]
pub fn generate_rectangles(
    rects: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    seed: i32,
) -> Vec<f64> {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut out: Vec<f64> = Vec::new();
    fill_rectangles(&mut out, rects, &o, seed);
    out
}

// Persistent output buffer so the data outlives the call and isn't freed on return.
thread_local! {
    static OUT: std::cell::RefCell<Vec<f64>> = std::cell::RefCell::new(Vec::new());
}

/// Zero-copy API: fills a persistent buffer and returns a Float64Array *view* over
/// WASM linear memory (no copy across the boundary). The view is only valid until the
/// next call (which mutates/reallocates the buffer) or any other WASM memory growth —
/// the caller must read/draw from it before calling again, and must not retain it.
#[wasm_bindgen]
pub fn generate_rectangles_view(
    rects: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    seed: i32,
) -> js_sys::Float64Array {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        fill_rectangles(&mut out, rects, &o, seed);
        // SAFETY: the view aliases the persistent OUT buffer in WASM memory. It is
        // invalidated by the next call or any memory growth; the JS caller reads it
        // immediately and does not retain it.
        unsafe { js_sys::Float64Array::view(&out) }
    })
}

/// Pure-compute benchmark: generates ops into a reused scratch buffer and returns a
/// checksum (sum of all emitted coordinates) so the optimizer can't elide the work.
/// Isolates the algorithm from the JS<->WASM marshalling cost of the big buffer.
#[wasm_bindgen]
pub fn bench_generate(
    rects: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    seed: i32,
) -> f64 {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut rng = Random { seed };
    let count = rects.len() / 4;
    let mut scratch: Vec<f64> = Vec::with_capacity(16 * OP_STRIDE);
    let mut checksum = 0.0;
    for i in 0..count {
        let b = i * 4;
        scratch.clear();
        rectangle_into(rects[b], rects[b + 1], rects[b + 2], rects[b + 3], &mut rng, &o, &mut scratch);
        for v in &scratch {
            checksum += *v;
        }
    }
    checksum
}
