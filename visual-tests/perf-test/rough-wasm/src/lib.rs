//! Minimal Rust/WASM port of RoughJS's `generator.rectangle` (stroke-only path).
//!
//! Faithful to src/renderer.ts: rectangle -> polygon -> linearPath -> _doubleLine
//! -> _line, driven by the Park-Miller LCG from src/math.ts. Only the 6 option
//! fields the stroke path actually reads are modeled (see `Options`).

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

// --- Options read by the rectangle + ellipse paths. The rectangle path ignores the
// curve_* fields; the ellipse path uses all of them. ---

#[derive(Clone, Copy)]
struct Options {
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    curve_step_count: f64,
    curve_fitting: f64,
    curve_tightness: f64,
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
fn offset(min: f64, max: f64, rng: &mut Random, o: &Options, roughness_gain: f64) -> f64 {
    o.roughness * roughness_gain * ((rng.next() * (max - min)) + min)
}

#[inline]
fn offset_opt(x: f64, rng: &mut Random, o: &Options, roughness_gain: f64) -> f64 {
    offset(-x, x, rng, o, roughness_gain)
}

/// Port of renderer.ts `_line`. Appends a `move` + a `bcurveTo` op into `out`.
fn line(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rng: &mut Random,
    o: &Options,
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
    o: &Options,
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
    o: &Options,
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
) -> Options {
    Options {
        roughness,
        max_randomness_offset,
        bowing,
        preserve_vertices,
        disable_multi_stroke,
        // Unused by the rectangle path.
        curve_step_count: 0.0,
        curve_fitting: 0.0,
        curve_tightness: 0.0,
    }
}

// --- Op emitters into the flat stride-7 buffer ---

#[inline]
fn emit_move(out: &mut Vec<f64>, x: f64, y: f64) {
    out.extend_from_slice(&[OpType::Move as u8 as f64, x, y, 0.0, 0.0, 0.0, 0.0]);
}

#[inline]
fn emit_bcurve(out: &mut Vec<f64>, c1x: f64, c1y: f64, c2x: f64, c2y: f64, ex: f64, ey: f64) {
    out.extend_from_slice(&[OpType::BCurveTo as u8 as f64, c1x, c1y, c2x, c2y, ex, ey]);
}

#[inline]
fn emit_line(out: &mut Vec<f64>, x: f64, y: f64) {
    out.extend_from_slice(&[OpType::LineTo as u8 as f64, x, y, 0.0, 0.0, 0.0, 0.0]);
}

// --- Ellipse path (renderer.ts generateEllipseParams / _computeEllipsePoints / _curve) ---

struct EllipseParams {
    increment: f64,
    rx: f64,
    ry: f64,
}

/// Port of renderer.ts `generateEllipseParams`.
fn generate_ellipse_params(width: f64, height: f64, rng: &mut Random, o: &Options) -> EllipseParams {
    use std::f64::consts::PI;
    let psq = (PI * 2.0 * (((width / 2.0).powi(2) + (height / 2.0).powi(2)) / 2.0).sqrt()).sqrt();
    let step_count = o
        .curve_step_count
        .max((o.curve_step_count / (200.0_f64).sqrt()) * psq)
        .ceil();
    let increment = (PI * 2.0) / step_count;
    let mut rx = (width / 2.0).abs();
    let mut ry = (height / 2.0).abs();
    let curve_fit_randomness = 1.0 - o.curve_fitting;
    rx += offset_opt(rx * curve_fit_randomness, rng, o, 1.0);
    ry += offset_opt(ry * curve_fit_randomness, rng, o, 1.0);
    EllipseParams { increment, rx, ry }
}

/// Port of renderer.ts `_computeEllipsePoints`, returning only `allPoints` (corePoints
/// are used for fill estimation, which the stroke-only path doesn't need; skipping them
/// does not change the RNG call sequence).
fn compute_ellipse_points(
    increment: f64,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    offset_factor: f64,
    overlap: f64,
    rng: &mut Random,
    o: &Options,
) -> Vec<(f64, f64)> {
    use std::f64::consts::PI;
    let mut all: Vec<(f64, f64)> = Vec::new();
    if o.roughness == 0.0 {
        let inc = increment / 4.0;
        all.push((cx + rx * (-inc).cos(), cy + ry * (-inc).sin()));
        let mut angle = 0.0;
        while angle <= PI * 2.0 {
            all.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
            angle += inc;
        }
        all.push((cx + rx, cy)); // cos(0)=1, sin(0)=0
        all.push((cx + rx * inc.cos(), cy + ry * inc.sin()));
    } else {
        let rad_offset = offset_opt(0.5, rng, o, 1.0) - (PI / 2.0);
        all.push((
            offset_opt(offset_factor, rng, o, 1.0) + cx + 0.9 * rx * (rad_offset - increment).cos(),
            offset_opt(offset_factor, rng, o, 1.0) + cy + 0.9 * ry * (rad_offset - increment).sin(),
        ));
        let end_angle = PI * 2.0 + rad_offset - 0.01;
        let mut angle = rad_offset;
        while angle < end_angle {
            all.push((
                offset_opt(offset_factor, rng, o, 1.0) + cx + rx * angle.cos(),
                offset_opt(offset_factor, rng, o, 1.0) + cy + ry * angle.sin(),
            ));
            angle += increment;
        }
        all.push((
            offset_opt(offset_factor, rng, o, 1.0) + cx + rx * (rad_offset + PI * 2.0 + overlap * 0.5).cos(),
            offset_opt(offset_factor, rng, o, 1.0) + cy + ry * (rad_offset + PI * 2.0 + overlap * 0.5).sin(),
        ));
        all.push((
            offset_opt(offset_factor, rng, o, 1.0) + cx + 0.98 * rx * (rad_offset + overlap).cos(),
            offset_opt(offset_factor, rng, o, 1.0) + cy + 0.98 * ry * (rad_offset + overlap).sin(),
        ));
        all.push((
            offset_opt(offset_factor, rng, o, 1.0) + cx + 0.9 * rx * (rad_offset + overlap * 0.5).cos(),
            offset_opt(offset_factor, rng, o, 1.0) + cy + 0.9 * ry * (rad_offset + overlap * 0.5).sin(),
        ));
    }
    all
}

/// Port of renderer.ts `_curve` (closePoint = None for the ellipse stroke path).
fn curve(points: &[(f64, f64)], rng: &mut Random, o: &Options, out: &mut Vec<f64>) {
    let len = points.len();
    if len > 3 {
        let s = 1.0 - o.curve_tightness;
        emit_move(out, points[1].0, points[1].1);
        let mut i = 1;
        while i + 2 < len {
            let b1x = points[i].0 + (s * points[i + 1].0 - s * points[i - 1].0) / 6.0;
            let b1y = points[i].1 + (s * points[i + 1].1 - s * points[i - 1].1) / 6.0;
            let b2x = points[i + 1].0 + (s * points[i].0 - s * points[i + 2].0) / 6.0;
            let b2y = points[i + 1].1 + (s * points[i].1 - s * points[i + 2].1) / 6.0;
            emit_bcurve(out, b1x, b1y, b2x, b2y, points[i + 1].0, points[i + 1].1);
            i += 1;
        }
        // closePoint is None for ellipse -> no trailing lineTo.
    } else if len == 3 {
        emit_move(out, points[1].0, points[1].1);
        emit_bcurve(
            out,
            points[1].0,
            points[1].1,
            points[2].0,
            points[2].1,
            points[2].0,
            points[2].1,
        );
    } else if len == 2 {
        // Degenerate (won't occur for real ellipses); emit a sketchy line for fidelity.
        line(points[0].0, points[0].1, points[1].0, points[1].1, rng, o, true, true, out);
    }
}

/// One ellipse outline. Appends ops to `out`. Port of renderer.ts `ellipseWithParams`.
fn ellipse_into(
    x: f64,
    y: f64,
    params: &EllipseParams,
    rng: &mut Random,
    o: &Options,
    out: &mut Vec<f64>,
) {
    // overlap = increment * _offset(0.1, _offset(0.4, 1, o), o) -- inner offset first.
    let inner = offset(0.4, 1.0, rng, o, 1.0);
    let overlap = params.increment * offset(0.1, inner, rng, o, 1.0);

    let ap1 = compute_ellipse_points(params.increment, x, y, params.rx, params.ry, 1.0, overlap, rng, o);
    curve(&ap1, rng, o, out);

    if !o.disable_multi_stroke && o.roughness != 0.0 {
        let ap2 = compute_ellipse_points(params.increment, x, y, params.rx, params.ry, 1.5, 0.0, rng, o);
        curve(&ap2, rng, o, out);
    }
}

fn fill_ellipses(out: &mut Vec<f64>, ellipses: &[f64], o: &Options, seed: i32) {
    out.clear();
    let mut rng = Random { seed };
    let count = ellipses.len() / 4;
    for i in 0..count {
        let b = i * 4;
        let params = generate_ellipse_params(ellipses[b + 2], ellipses[b + 3], &mut rng, o);
        ellipse_into(ellipses[b], ellipses[b + 1], &params, &mut rng, o, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn make_ellipse_opts(
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    curve_step_count: f64,
    curve_fitting: f64,
    curve_tightness: f64,
) -> Options {
    Options {
        roughness,
        max_randomness_offset,
        bowing,
        preserve_vertices,
        disable_multi_stroke,
        curve_step_count,
        curve_fitting,
        curve_tightness,
    }
}

/// Copy-based ellipse API (returns Vec<f64>, copied across the boundary).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_ellipses(
    ellipses: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    curve_step_count: f64,
    curve_fitting: f64,
    curve_tightness: f64,
    seed: i32,
) -> Vec<f64> {
    let o = make_ellipse_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke, curve_step_count, curve_fitting, curve_tightness);
    let mut out: Vec<f64> = Vec::new();
    fill_ellipses(&mut out, ellipses, &o, seed);
    out
}

/// Zero-copy ellipse API (Float64Array view over WASM memory; see generate_rectangles_view).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_ellipses_view(
    ellipses: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    curve_step_count: f64,
    curve_fitting: f64,
    curve_tightness: f64,
    seed: i32,
) -> js_sys::Float64Array {
    let o = make_ellipse_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke, curve_step_count, curve_fitting, curve_tightness);
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        fill_ellipses(&mut out, ellipses, &o, seed);
        // SAFETY: see generate_rectangles_view.
        unsafe { js_sys::Float64Array::view(&out) }
    })
}

/// Pure-compute ellipse benchmark (returns a checksum; no buffer marshalled).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn bench_ellipses(
    ellipses: &[f64],
    roughness: f64,
    max_randomness_offset: f64,
    bowing: f64,
    preserve_vertices: bool,
    disable_multi_stroke: bool,
    curve_step_count: f64,
    curve_fitting: f64,
    curve_tightness: f64,
    seed: i32,
) -> f64 {
    let o = make_ellipse_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke, curve_step_count, curve_fitting, curve_tightness);
    let mut rng = Random { seed };
    let count = ellipses.len() / 4;
    let mut scratch: Vec<f64> = Vec::new();
    let mut checksum = 0.0;
    for i in 0..count {
        let b = i * 4;
        scratch.clear();
        let params = generate_ellipse_params(ellipses[b + 2], ellipses[b + 3], &mut rng, &o);
        ellipse_into(ellipses[b], ellipses[b + 1], &params, &mut rng, &o, &mut scratch);
        for v in &scratch {
            checksum += *v;
        }
    }
    checksum
}

// --- line / linearPath / polygon (renderer.ts line / linearPath / polygon) ---

/// Port of renderer.ts `linearPath`.
fn linear_path_into(points: &[(f64, f64)], close: bool, rng: &mut Random, o: &Options, out: &mut Vec<f64>) {
    let len = points.len();
    if len > 2 {
        for i in 0..(len - 1) {
            double_line(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1, rng, o, out);
        }
        if close {
            double_line(points[len - 1].0, points[len - 1].1, points[0].0, points[0].1, rng, o, out);
        }
    } else if len == 2 {
        // renderer.ts: linearPath(len==2) delegates to line() == _doubleLine.
        double_line(points[0].0, points[0].1, points[1].0, points[1].1, rng, o, out);
    }
}

/// `coords` is a flat [x1,y1,x2,y2, ...] buffer — one line segment per 4 values.
fn fill_lines(out: &mut Vec<f64>, coords: &[f64], o: &Options, seed: i32) {
    out.clear();
    let mut rng = Random { seed };
    let n = coords.len() / 4;
    for i in 0..n {
        let b = i * 4;
        double_line(coords[b], coords[b + 1], coords[b + 2], coords[b + 3], &mut rng, o, out);
    }
}

/// `points` is a flat [x,y, ...] buffer of `verts`-vertex polygons laid end to end.
fn fill_polygons(out: &mut Vec<f64>, points: &[f64], verts: usize, o: &Options, seed: i32) {
    out.clear();
    let mut rng = Random { seed };
    if verts == 0 {
        return;
    }
    let n = points.len() / (verts * 2);
    let mut buf: Vec<(f64, f64)> = Vec::with_capacity(verts);
    for p in 0..n {
        buf.clear();
        let base = p * verts * 2;
        for v in 0..verts {
            buf.push((points[base + v * 2], points[base + v * 2 + 1]));
        }
        linear_path_into(&buf, true, &mut rng, o, out);
    }
}

// --- SVG path: parser + absolutize + normalize + svgPath (path-data-parser + renderer.ts) ---

struct Seg {
    key: u8, // ASCII command
    data: Vec<f64>,
}

fn params_count(c: u8) -> i32 {
    match c {
        b'A' | b'a' => 7,
        b'C' | b'c' => 6,
        b'H' | b'h' => 1,
        b'L' | b'l' => 2,
        b'M' | b'm' => 2,
        b'Q' | b'q' => 4,
        b'S' | b's' => 4,
        b'T' | b't' => 2,
        b'V' | b'v' => 1,
        b'Z' | b'z' => 0,
        _ => -1,
    }
}

enum Tok {
    Cmd(u8),
    Num(f64),
}

/// Length of a leading SVG number, matching path-data-parser's tokenizer regex.
fn scan_number(b: &[u8]) -> Option<usize> {
    let n = b.len();
    let mut i = 0;
    if i < n && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut has_int = false;
    while i < n && b[i].is_ascii_digit() {
        i += 1;
        has_int = true;
    }
    if has_int {
        if i < n && b[i] == b'.' {
            i += 1;
            while i < n && b[i].is_ascii_digit() {
                i += 1;
            }
        }
    } else if i < n && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < n && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == fs {
            return None;
        }
    } else {
        return None;
    }
    if i < n && (b[i] == b'e' || b[i] == b'E') {
        let save = i;
        i += 1;
        if i < n && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let es = i;
        while i < n && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            i = save; // exponent needs digits; otherwise exclude it
        }
    }
    Some(i)
}

fn tokenize(d: &str) -> Option<Vec<Tok>> {
    let b = d.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut toks = Vec::new();
    while i < n {
        let c = b[i];
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' || c == b',' {
            i += 1;
        } else if c.is_ascii_alphabetic() && params_count(c) >= 0 {
            toks.push(Tok::Cmd(c));
            i += 1;
        } else if let Some(len) = scan_number(&b[i..]) {
            let v: f64 = d[i..i + len].parse().ok()?;
            toks.push(Tok::Num(v));
            i += len;
        } else {
            return None; // matches JS tokenize() returning []
        }
    }
    Some(toks)
}

/// Port of path-data-parser `parsePath`.
fn parse_path(d: &str) -> Vec<Seg> {
    let toks = tokenize(d).unwrap_or_default();
    let len = toks.len();
    let mut segments: Vec<Seg> = Vec::new();
    let mut mode: u8 = 0; // 0 == BOD sentinel
    let mut index = 0usize;
    while index < len {
        let params_n;
        match &toks[index] {
            _ if mode == 0 => match &toks[index] {
                Tok::Cmd(c) if *c == b'M' || *c == b'm' => {
                    index += 1;
                    params_n = params_count(*c);
                    mode = *c;
                }
                _ => {
                    let mut s = String::from("M0,0");
                    s.push_str(d);
                    return parse_path(&s);
                }
            },
            Tok::Num(_) => {
                params_n = params_count(mode);
            }
            Tok::Cmd(c) => {
                index += 1;
                params_n = params_count(*c);
                mode = *c;
            }
        }
        // JS: (index + paramsCount) < tokens.length, tokens includes EOD (len+1).
        if (index as i32 + params_n) as usize <= len {
            let mut params: Vec<f64> = Vec::with_capacity(params_n.max(0) as usize);
            for t in toks.iter().skip(index).take(params_n as usize) {
                if let Tok::Num(v) = t {
                    params.push(*v);
                } else {
                    return segments; // JS throws; we stop
                }
            }
            segments.push(Seg { key: mode, data: params });
            index += params_n as usize;
            if mode == b'M' {
                mode = b'L';
            }
            if mode == b'm' {
                mode = b'l';
            }
        } else {
            break; // "Path data ended short"
        }
    }
    segments
}

/// Port of path-data-parser `absolutize`.
fn absolutize(segments: &[Seg]) -> Vec<Seg> {
    let (mut cx, mut cy) = (0.0, 0.0);
    let (mut subx, mut suby) = (0.0, 0.0);
    let mut out: Vec<Seg> = Vec::new();
    for s in segments {
        let d = &s.data;
        match s.key {
            b'M' => {
                out.push(Seg { key: b'M', data: d.clone() });
                cx = d[0];
                cy = d[1];
                subx = d[0];
                suby = d[1];
            }
            b'm' => {
                cx += d[0];
                cy += d[1];
                out.push(Seg { key: b'M', data: vec![cx, cy] });
                subx = cx;
                suby = cy;
            }
            b'L' => {
                out.push(Seg { key: b'L', data: d.clone() });
                cx = d[0];
                cy = d[1];
            }
            b'l' => {
                cx += d[0];
                cy += d[1];
                out.push(Seg { key: b'L', data: vec![cx, cy] });
            }
            b'C' => {
                out.push(Seg { key: b'C', data: d.clone() });
                cx = d[4];
                cy = d[5];
            }
            b'c' => {
                let nd: Vec<f64> = d.iter().enumerate().map(|(i, v)| if i % 2 == 1 { v + cy } else { v + cx }).collect();
                cx = nd[4];
                cy = nd[5];
                out.push(Seg { key: b'C', data: nd });
            }
            b'Q' => {
                out.push(Seg { key: b'Q', data: d.clone() });
                cx = d[2];
                cy = d[3];
            }
            b'q' => {
                let nd: Vec<f64> = d.iter().enumerate().map(|(i, v)| if i % 2 == 1 { v + cy } else { v + cx }).collect();
                cx = nd[2];
                cy = nd[3];
                out.push(Seg { key: b'Q', data: nd });
            }
            b'A' => {
                out.push(Seg { key: b'A', data: d.clone() });
                cx = d[5];
                cy = d[6];
            }
            b'a' => {
                cx += d[5];
                cy += d[6];
                out.push(Seg { key: b'A', data: vec![d[0], d[1], d[2], d[3], d[4], cx, cy] });
            }
            b'H' => {
                out.push(Seg { key: b'H', data: d.clone() });
                cx = d[0];
            }
            b'h' => {
                cx += d[0];
                out.push(Seg { key: b'H', data: vec![cx] });
            }
            b'V' => {
                out.push(Seg { key: b'V', data: d.clone() });
                cy = d[0];
            }
            b'v' => {
                cy += d[0];
                out.push(Seg { key: b'V', data: vec![cy] });
            }
            b'S' => {
                out.push(Seg { key: b'S', data: d.clone() });
                cx = d[2];
                cy = d[3];
            }
            b's' => {
                let nd: Vec<f64> = d.iter().enumerate().map(|(i, v)| if i % 2 == 1 { v + cy } else { v + cx }).collect();
                cx = nd[2];
                cy = nd[3];
                out.push(Seg { key: b'S', data: nd });
            }
            b'T' => {
                out.push(Seg { key: b'T', data: d.clone() });
                cx = d[0];
                cy = d[1];
            }
            b't' => {
                cx += d[0];
                cy += d[1];
                out.push(Seg { key: b'T', data: vec![cx, cy] });
            }
            b'Z' | b'z' => {
                out.push(Seg { key: b'Z', data: vec![] });
                cx = subx;
                cy = suby;
            }
            _ => {}
        }
    }
    out
}

#[inline]
fn rotate(x: f64, y: f64, angle: f64) -> (f64, f64) {
    (x * angle.cos() - y * angle.sin(), x * angle.sin() + y * angle.cos())
}

#[inline]
fn round9(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

/// Port of path-data-parser arc-to-cubic, returning control points in the rotated frame.
#[allow(clippy::too_many_arguments)]
fn arc_points(
    mut x1: f64,
    mut y1: f64,
    mut x2: f64,
    mut y2: f64,
    mut r1: f64,
    mut r2: f64,
    angle_deg: f64,
    large: bool,
    sweep: bool,
    recursive: Option<(f64, f64, f64, f64)>,
) -> Vec<[f64; 2]> {
    use std::f64::consts::PI;
    let angle_rad = PI * angle_deg / 180.0;
    let (f1, mut f2, cx, cy);
    if let Some((rf1, rf2, rcx, rcy)) = recursive {
        f1 = rf1;
        f2 = rf2;
        cx = rcx;
        cy = rcy;
    } else {
        let p1 = rotate(x1, y1, -angle_rad);
        x1 = p1.0;
        y1 = p1.1;
        let p2 = rotate(x2, y2, -angle_rad);
        x2 = p2.0;
        y2 = p2.1;
        let x = (x1 - x2) / 2.0;
        let y = (y1 - y2) / 2.0;
        let mut h = (x * x) / (r1 * r1) + (y * y) / (r2 * r2);
        if h > 1.0 {
            h = h.sqrt();
            r1 *= h;
            r2 *= h;
        }
        let sign = if large == sweep { -1.0 } else { 1.0 };
        let r1p = r1 * r1;
        let r2p = r2 * r2;
        let left = r1p * r2p - r1p * y * y - r2p * x * x;
        let right = r1p * y * y + r2p * x * x;
        let k = sign * (left / right).abs().sqrt();
        let cxv = k * r1 * y / r2 + (x1 + x2) / 2.0;
        let cyv = k * -r2 * x / r1 + (y1 + y2) / 2.0;
        let mut f1v = round9((y1 - cyv) / r2).asin();
        let mut f2v = round9((y2 - cyv) / r2).asin();
        if x1 < cxv {
            f1v = PI - f1v;
        }
        if x2 < cxv {
            f2v = PI - f2v;
        }
        if f1v < 0.0 {
            f1v += PI * 2.0;
        }
        if f2v < 0.0 {
            f2v += PI * 2.0;
        }
        if sweep && f1v > f2v {
            f1v -= PI * 2.0;
        }
        if !sweep && f2v > f1v {
            f2v -= PI * 2.0;
        }
        f1 = f1v;
        f2 = f2v;
        cx = cxv;
        cy = cyv;
    }
    let mut df = f2 - f1;
    let mut params: Vec<[f64; 2]> = Vec::new();
    if df.abs() > (PI * 120.0 / 180.0) {
        let f2old = f2;
        let x2old = x2;
        let y2old = y2;
        if sweep && f2 > f1 {
            f2 = f1 + (PI * 120.0 / 180.0);
        } else {
            f2 = f1 - (PI * 120.0 / 180.0);
        }
        x2 = cx + r1 * f2.cos();
        y2 = cy + r2 * f2.sin();
        params = arc_points(x2, y2, x2old, y2old, r1, r2, angle_deg, false, sweep, Some((f2, f2old, cx, cy)));
    }
    df = f2 - f1;
    let c1 = f1.cos();
    let s1 = f1.sin();
    let c2 = f2.cos();
    let s2 = f2.sin();
    let t = (df / 4.0).tan();
    let hx = 4.0 / 3.0 * r1 * t;
    let hy = 4.0 / 3.0 * r2 * t;
    let m1 = [x1, y1];
    let mut m2 = [x1 + hx * s1, y1 - hy * c1];
    let m3 = [x2 + hx * s2, y2 - hy * c2];
    let m4 = [x2, y2];
    m2[0] = 2.0 * m1[0] - m2[0];
    m2[1] = 2.0 * m1[1] - m2[1];
    let mut out = vec![m2, m3, m4];
    out.extend(params);
    out
}

#[allow(clippy::too_many_arguments)]
fn arc_to_cubic_curves(x1: f64, y1: f64, x2: f64, y2: f64, r1: f64, r2: f64, angle_deg: f64, large: bool, sweep: bool) -> Vec<[f64; 6]> {
    use std::f64::consts::PI;
    let angle_rad = PI * angle_deg / 180.0;
    let pts = arc_points(x1, y1, x2, y2, r1, r2, angle_deg, large, sweep, None);
    let mut curves: Vec<[f64; 6]> = Vec::new();
    let mut i = 0;
    while i + 3 <= pts.len() {
        let a = rotate(pts[i][0], pts[i][1], angle_rad);
        let b = rotate(pts[i + 1][0], pts[i + 1][1], angle_rad);
        let c = rotate(pts[i + 2][0], pts[i + 2][1], angle_rad);
        curves.push([a.0, a.1, b.0, b.1, c.0, c.1]);
        i += 3;
    }
    curves
}

/// Port of path-data-parser `normalize` (outputs only M, L, C, Z).
fn normalize(segments: &[Seg]) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let mut last_type: u8 = 0;
    let (mut cx, mut cy) = (0.0, 0.0);
    let (mut subx, mut suby) = (0.0, 0.0);
    let (mut lcx, mut lcy) = (0.0, 0.0);
    for s in segments {
        let d = &s.data;
        match s.key {
            b'M' => {
                out.push(Seg { key: b'M', data: d.clone() });
                cx = d[0];
                cy = d[1];
                subx = d[0];
                suby = d[1];
            }
            b'C' => {
                out.push(Seg { key: b'C', data: d.clone() });
                cx = d[4];
                cy = d[5];
                lcx = d[2];
                lcy = d[3];
            }
            b'L' => {
                out.push(Seg { key: b'L', data: d.clone() });
                cx = d[0];
                cy = d[1];
            }
            b'H' => {
                cx = d[0];
                out.push(Seg { key: b'L', data: vec![cx, cy] });
            }
            b'V' => {
                cy = d[0];
                out.push(Seg { key: b'L', data: vec![cx, cy] });
            }
            b'S' => {
                let (cx1, cy1) = if last_type == b'C' || last_type == b'S' {
                    (cx + (cx - lcx), cy + (cy - lcy))
                } else {
                    (cx, cy)
                };
                out.push(Seg { key: b'C', data: vec![cx1, cy1, d[0], d[1], d[2], d[3]] });
                lcx = d[0];
                lcy = d[1];
                cx = d[2];
                cy = d[3];
            }
            b'T' => {
                let (x, y) = (d[0], d[1]);
                let (x1, y1) = if last_type == b'Q' || last_type == b'T' {
                    (cx + (cx - lcx), cy + (cy - lcy))
                } else {
                    (cx, cy)
                };
                let cx1 = cx + 2.0 * (x1 - cx) / 3.0;
                let cy1 = cy + 2.0 * (y1 - cy) / 3.0;
                let cx2 = x + 2.0 * (x1 - x) / 3.0;
                let cy2 = y + 2.0 * (y1 - y) / 3.0;
                out.push(Seg { key: b'C', data: vec![cx1, cy1, cx2, cy2, x, y] });
                lcx = x1;
                lcy = y1;
                cx = x;
                cy = y;
            }
            b'Q' => {
                let (x1, y1, x, y) = (d[0], d[1], d[2], d[3]);
                let cx1 = cx + 2.0 * (x1 - cx) / 3.0;
                let cy1 = cy + 2.0 * (y1 - cy) / 3.0;
                let cx2 = x + 2.0 * (x1 - x) / 3.0;
                let cy2 = y + 2.0 * (y1 - y) / 3.0;
                out.push(Seg { key: b'C', data: vec![cx1, cy1, cx2, cy2, x, y] });
                lcx = x1;
                lcy = y1;
                cx = x;
                cy = y;
            }
            b'A' => {
                let r1 = d[0].abs();
                let r2 = d[1].abs();
                let angle = d[2];
                let large = d[3] != 0.0;
                let sweep = d[4] != 0.0;
                let x = d[5];
                let y = d[6];
                if r1 == 0.0 || r2 == 0.0 {
                    out.push(Seg { key: b'C', data: vec![cx, cy, x, y, x, y] });
                    cx = x;
                    cy = y;
                } else if cx != x || cy != y {
                    for c in arc_to_cubic_curves(cx, cy, x, y, r1, r2, angle, large, sweep) {
                        out.push(Seg { key: b'C', data: c.to_vec() });
                    }
                    cx = x;
                    cy = y;
                }
            }
            b'Z' => {
                out.push(Seg { key: b'Z', data: vec![] });
                cx = subx;
                cy = suby;
            }
            _ => {}
        }
        last_type = s.key;
    }
    out
}

/// Port of renderer.ts `_bezierTo`.
#[allow(clippy::too_many_arguments)]
fn bezier_to(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64, current: (f64, f64), rng: &mut Random, o: &Options, out: &mut Vec<f64>) {
    let base = if o.max_randomness_offset != 0.0 { o.max_randomness_offset } else { 1.0 };
    let ros = [base, base + 0.3];
    let iterations = if o.disable_multi_stroke { 1 } else { 2 };
    let pv = o.preserve_vertices;
    for i in 0..iterations {
        if i == 0 {
            emit_move(out, current.0, current.1);
        } else {
            emit_move(
                out,
                current.0 + if pv { 0.0 } else { offset_opt(ros[0], rng, o, 1.0) },
                current.1 + if pv { 0.0 } else { offset_opt(ros[0], rng, o, 1.0) },
            );
        }
        let f = if pv {
            (x, y)
        } else {
            (x + offset_opt(ros[i], rng, o, 1.0), y + offset_opt(ros[i], rng, o, 1.0))
        };
        emit_bcurve(
            out,
            x1 + offset_opt(ros[i], rng, o, 1.0),
            y1 + offset_opt(ros[i], rng, o, 1.0),
            x2 + offset_opt(ros[i], rng, o, 1.0),
            y2 + offset_opt(ros[i], rng, o, 1.0),
            f.0,
            f.1,
        );
    }
}

/// renderer.ts `svgPath`: emit ops for a (preprocessed) SVG path string.
fn svg_path_into(d: &str, rng: &mut Random, o: &Options, out: &mut Vec<f64>) {
    let segs = normalize(&absolutize(&parse_path(d)));
    let mut first = (0.0, 0.0);
    let mut current = (0.0, 0.0);
    for s in &segs {
        match s.key {
            b'M' => {
                current = (s.data[0], s.data[1]);
                first = current;
            }
            b'L' => {
                double_line(current.0, current.1, s.data[0], s.data[1], rng, o, out);
                current = (s.data[0], s.data[1]);
            }
            b'C' => {
                let d = &s.data;
                bezier_to(d[0], d[1], d[2], d[3], d[4], d[5], current, rng, o, out);
                current = (d[4], d[5]);
            }
            b'Z' => {
                double_line(current.0, current.1, first.0, first.1, rng, o, out);
                current = first;
            }
            _ => {}
        }
    }
}

/// generator.ts `path()` preprocessing: newline -> space, then drop one whitespace char
/// immediately after a '-' (the third JS replace is a no-op bug, so omitted).
fn preprocess_path(d: &str) -> String {
    let spaced = d.replace('\n', " ");
    let b = spaced.as_bytes();
    let mut out = String::with_capacity(spaced.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        out.push(c as char);
        if c == b'-' && i + 1 < b.len() {
            let nx = b[i + 1];
            if nx == b' ' || nx == b'\t' || nx == b'\r' || nx == b'\n' || nx == 0x0c || nx == 0x0b {
                i += 2; // skip the one whitespace after '-'
                continue;
            }
        }
        i += 1;
    }
    out
}

/// One SVG path's stroke ops (no fill, default simplification). Port of generator.path stroke.
fn path_into(d: &str, rng: &mut Random, o: &Options, out: &mut Vec<f64>) {
    if d.is_empty() {
        return;
    }
    let pd = preprocess_path(d);
    svg_path_into(&pd, rng, o, out);
}

fn fill_paths(out: &mut Vec<f64>, d: &str, repeat: usize, o: &Options, seed: i32) {
    out.clear();
    let mut rng = Random { seed };
    for _ in 0..repeat {
        path_into(d, &mut rng, o, out);
    }
}

// --- wasm exports for line / polygon / path (copy / view / pure) ---

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_lines(coords: &[f64], roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> Vec<f64> {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut out = Vec::new();
    fill_lines(&mut out, coords, &o, seed);
    out
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_lines_view(coords: &[f64], roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> js_sys::Float64Array {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        fill_lines(&mut out, coords, &o, seed);
        unsafe { js_sys::Float64Array::view(&out) }
    })
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn bench_lines(coords: &[f64], roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> f64 {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut scratch = Vec::new();
    fill_lines(&mut scratch, coords, &o, seed);
    scratch.iter().sum()
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_polygons(points: &[f64], verts: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> Vec<f64> {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut out = Vec::new();
    fill_polygons(&mut out, points, verts, &o, seed);
    out
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_polygons_view(points: &[f64], verts: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> js_sys::Float64Array {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        fill_polygons(&mut out, points, verts, &o, seed);
        unsafe { js_sys::Float64Array::view(&out) }
    })
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn bench_polygons(points: &[f64], verts: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> f64 {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut scratch = Vec::new();
    fill_polygons(&mut scratch, points, verts, &o, seed);
    scratch.iter().sum()
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_path(d: &str, repeat: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> Vec<f64> {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut out = Vec::new();
    fill_paths(&mut out, d, repeat, &o, seed);
    out
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn generate_path_view(d: &str, repeat: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> js_sys::Float64Array {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        fill_paths(&mut out, d, repeat, &o, seed);
        unsafe { js_sys::Float64Array::view(&out) }
    })
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn bench_path(d: &str, repeat: usize, roughness: f64, max_randomness_offset: f64, bowing: f64, preserve_vertices: bool, disable_multi_stroke: bool, seed: i32) -> f64 {
    let o = make_opts(roughness, max_randomness_offset, bowing, preserve_vertices, disable_multi_stroke);
    let mut scratch = Vec::new();
    fill_paths(&mut scratch, d, repeat, &o, seed);
    scratch.iter().sum()
}

/// Fill `out` with the op buffer for a batch of rectangles. `rects` is a flat
/// [x,y,w,h, ...] buffer. One shared RNG advances across all rects, mirroring how
/// rough.js reuses the randomizer on the shared options object.
fn fill_rectangles(out: &mut Vec<f64>, rects: &[f64], o: &Options, seed: i32) {
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
