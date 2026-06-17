import { describe, it, expect } from 'vitest';
import {
  ellipseImpls,
  referenceEllipse,
  DEFAULTS,
  FlatOptions,
  OP_STRIDE,
  OP_MOVE,
  OP_BCURVE,
  OP_LINE,
} from './fidelity-impls.js';

interface Case {
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  seed: number; // must be non-zero (seed 0 falls back to Math.random in rough.js)
  o?: Partial<FlatOptions>;
}

// x,y is the ellipse CENTRE in rough.js. Cover both the rough path and the roughness=0
// "core" path, plus the curve_* options and a range of sizes/seeds.
const cases: Case[] = [
  { name: 'circle', x: 200, y: 200, w: 120, h: 120, seed: 1 },
  { name: 'wide ellipse', x: 300, y: 300, w: 400, h: 120, seed: 42 },
  { name: 'tall ellipse', x: 150, y: 400, w: 80, h: 500, seed: 7 },
  { name: 'small (min step count)', x: 50, y: 50, w: 20, h: 25, seed: 9 },
  { name: 'large (higher step count)', x: 400, y: 400, w: 700, h: 700, seed: 11 },
  { name: 'roughness 0 (core path)', x: 200, y: 200, w: 150, h: 150, seed: 3, o: { roughness: 0 } },
  { name: 'high roughness', x: 200, y: 200, w: 150, h: 150, seed: 5, o: { roughness: 3 } },
  { name: 'high step count', x: 200, y: 200, w: 150, h: 150, seed: 5, o: { curveStepCount: 30 } },
  { name: 'curve fitting 1', x: 200, y: 200, w: 150, h: 150, seed: 5, o: { curveFitting: 1 } },
  { name: 'curve tightness', x: 200, y: 200, w: 150, h: 150, seed: 5, o: { curveTightness: 0.5 } },
  { name: 'single stroke', x: 200, y: 200, w: 150, h: 150, seed: 5, o: { disableMultiStroke: true } },
];

// The ellipse path uses sin/cos. WASM (Rust libm) and V8 differ by a few ULPs, so use a
// relative tolerance. The pure-JS impl uses V8 Math, so it should match near-exactly.
const ELLIPSE_EPS = 1e-9;

const optsFor = (c: Case): FlatOptions => ({ ...DEFAULTS, ...c.o });

for (const impl of ellipseImpls) {
  describe(`ellipse fidelity: ${impl.name}`, () => {
    for (const c of cases) {
      it(`matches rough.js — ${c.name}`, () => {
        const o = optsFor(c);
        const actual = impl.generate(c.x, c.y, c.w, c.h, o, c.seed);
        const expected = referenceEllipse(c.x, c.y, c.w, c.h, o, c.seed);
        expect(actual.length).toBe(expected.length);
        for (let i = 0; i < expected.length; i++) {
          if (i % OP_STRIDE === 0) {
            expect(actual[i]).toBe(expected[i]); // op code: exact
          } else {
            expect(Math.abs(actual[i] - expected[i])).toBeLessThanOrEqual(
              ELLIPSE_EPS * Math.max(1, Math.abs(expected[i]))
            );
          }
        }
      });
    }

    it('roughness=0 is seed-independent (core path adds no randomness)', () => {
      // At roughness 0 every offset is scaled by 0, so the output must not depend on seed.
      const a = impl.generate(200, 200, 150, 150, { ...DEFAULTS, roughness: 0 }, 3);
      const b = impl.generate(200, 200, 150, 150, { ...DEFAULTS, roughness: 0 }, 999);
      expect(Array.from(a)).toEqual(Array.from(b));
    });

    it('is deterministic for the same seed', () => {
      const a = impl.generate(220, 240, 160, 90, DEFAULTS, 1234);
      const b = impl.generate(220, 240, 160, 90, DEFAULTS, 1234);
      expect(Array.from(a)).toEqual(Array.from(b));
    });

    it('differs for different seeds', () => {
      const a = impl.generate(220, 240, 160, 90, DEFAULTS, 1);
      const b = impl.generate(220, 240, 160, 90, DEFAULTS, 2);
      expect(Array.from(a)).not.toEqual(Array.from(b));
    });

    it('is well-formed: stride-aligned, starts with a move, valid op codes', () => {
      const buf = impl.generate(200, 200, 120, 120, DEFAULTS, 1);
      expect(buf.length).toBeGreaterThan(0);
      expect(buf.length % OP_STRIDE).toBe(0);
      expect(buf[0]).toBe(OP_MOVE);
      for (let i = 0; i < buf.length; i += OP_STRIDE) {
        expect([OP_MOVE, OP_BCURVE, OP_LINE]).toContain(buf[i]);
      }
    });
  });
}
