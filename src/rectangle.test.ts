import { describe, it, expect } from 'vitest';
import {
  rectangleImpls,
  referenceRectangle,
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

// Cover the roughnessGain branches in _line (length <200, 200-500, >500), each option
// the rectangle path reads, and a range of seeds.
const cases: Case[] = [
  { name: 'small square (len <200)', x: 10, y: 10, w: 80, h: 80, seed: 1 },
  { name: 'wide rect', x: 5, y: 5, w: 480, h: 120, seed: 42 },
  { name: 'tall rect', x: 300, y: 20, w: 60, h: 600, seed: 7 },
  { name: 'long edge (>500 gain branch)', x: 0, y: 0, w: 700, h: 30, seed: 99 },
  { name: 'mid edge (200-500 gain branch)', x: 0, y: 0, w: 350, h: 350, seed: 123 },
  { name: 'high roughness', x: 50, y: 50, w: 100, h: 100, seed: 5, o: { roughness: 3 } },
  { name: 'low roughness', x: 50, y: 50, w: 100, h: 100, seed: 5, o: { roughness: 0.3 } },
  { name: 'preserveVertices', x: 20, y: 20, w: 200, h: 150, seed: 55, o: { preserveVertices: true } },
  { name: 'single stroke', x: 20, y: 20, w: 200, h: 150, seed: 55, o: { disableMultiStroke: true } },
  { name: 'no bowing', x: 20, y: 20, w: 200, h: 150, seed: 55, o: { bowing: 0 } },
];

// The rectangle path uses no trig, so every impl should match the rough.js reference to
// ~machine epsilon (differences only from float-op reordering across compilers).
const RECT_EPS = 1e-9;

const optsFor = (c: Case): FlatOptions => ({ ...DEFAULTS, ...c.o });

for (const impl of rectangleImpls) {
  describe(`rectangle fidelity: ${impl.name}`, () => {
    for (const c of cases) {
      it(`matches rough.js — ${c.name}`, () => {
        const o = optsFor(c);
        const actual = impl.generate(c.x, c.y, c.w, c.h, o, c.seed);
        const expected = referenceRectangle(c.x, c.y, c.w, c.h, o, c.seed);
        expect(actual.length).toBe(expected.length);
        for (let i = 0; i < expected.length; i++) {
          if (i % OP_STRIDE === 0) {
            expect(actual[i]).toBe(expected[i]); // op code: exact
          } else {
            expect(Math.abs(actual[i] - expected[i])).toBeLessThanOrEqual(
              RECT_EPS * Math.max(1, Math.abs(expected[i]))
            );
          }
        }
      });
    }

    it('emits 16 ops (4 sides x 2 strokes x 2 ops)', () => {
      const buf = impl.generate(10, 10, 80, 80, DEFAULTS, 1);
      expect(buf.length).toBe(16 * OP_STRIDE);
    });

    it('emits 8 ops when multi-stroke is disabled', () => {
      const buf = impl.generate(10, 10, 80, 80, { ...DEFAULTS, disableMultiStroke: true }, 1);
      expect(buf.length).toBe(8 * OP_STRIDE);
    });

    it('is deterministic for the same seed', () => {
      const a = impl.generate(12, 34, 56, 78, DEFAULTS, 1234);
      const b = impl.generate(12, 34, 56, 78, DEFAULTS, 1234);
      expect(Array.from(a)).toEqual(Array.from(b));
    });

    it('differs for different seeds', () => {
      const a = impl.generate(12, 34, 56, 78, DEFAULTS, 1);
      const b = impl.generate(12, 34, 56, 78, DEFAULTS, 2);
      expect(Array.from(a)).not.toEqual(Array.from(b));
    });

    it('is well-formed: stride-aligned, starts with a move, valid op codes', () => {
      const buf = impl.generate(10, 10, 80, 80, DEFAULTS, 1);
      expect(buf.length % OP_STRIDE).toBe(0);
      expect(buf[0]).toBe(OP_MOVE);
      for (let i = 0; i < buf.length; i += OP_STRIDE) {
        expect([OP_MOVE, OP_BCURVE, OP_LINE]).toContain(buf[i]);
      }
    });
  });
}
