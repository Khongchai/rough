import { describe, it, expect } from 'vitest';
import {
  polygonImpls,
  referencePolygon,
  DEFAULTS,
  FlatOptions,
  OP_STRIDE,
  OP_MOVE,
  OP_BCURVE,
  OP_LINE,
} from './fidelity-impls.js';

interface Case {
  name: string;
  points: number[]; // flat [x0,y0, x1,y1, ...]
  seed: number; // non-zero
  o?: Partial<FlatOptions>;
}

const cases: Case[] = [
  { name: 'triangle', points: [50, 10, 90, 90, 10, 90], seed: 1 },
  { name: 'square', points: [20, 20, 120, 20, 120, 120, 20, 120], seed: 42 },
  { name: 'pentagon', points: [100, 10, 160, 60, 140, 130, 60, 130, 40, 60], seed: 7 },
  { name: 'long edges (gain branches)', points: [0, 0, 700, 0, 350, 400], seed: 99 },
  { name: 'high roughness', points: [50, 10, 90, 90, 10, 90], seed: 5, o: { roughness: 3 } },
  { name: 'preserveVertices', points: [50, 10, 90, 90, 10, 90], seed: 5, o: { preserveVertices: true } },
  { name: 'single stroke', points: [50, 10, 90, 90, 10, 90], seed: 5, o: { disableMultiStroke: true } },
  { name: 'two points (degenerate)', points: [10, 10, 200, 200], seed: 3 },
];

const EPS = 1e-9; // no trig on the polygon path

const optsFor = (c: Case): FlatOptions => ({ ...DEFAULTS, ...c.o });

for (const impl of polygonImpls) {
  describe(`polygon fidelity: ${impl.name}`, () => {
    for (const c of cases) {
      it(`matches rough.js — ${c.name}`, () => {
        const o = optsFor(c);
        const actual = impl.generate(c.points, o, c.seed);
        const expected = referencePolygon(c.points, o, c.seed);
        expect(actual.length).toBe(expected.length);
        for (let i = 0; i < expected.length; i++) {
          if (i % OP_STRIDE === 0) expect(actual[i]).toBe(expected[i]);
          else expect(Math.abs(actual[i] - expected[i])).toBeLessThanOrEqual(EPS * Math.max(1, Math.abs(expected[i])));
        }
      });
    }

    it('triangle emits 12 ops (3 closed edges x 2 strokes x 2 ops)', () => {
      expect(impl.generate([50, 10, 90, 90, 10, 90], DEFAULTS, 1).length).toBe(12 * OP_STRIDE);
    });

    it('is deterministic and seed-sensitive', () => {
      const sq = [20, 20, 120, 20, 120, 120, 20, 120];
      const a = impl.generate(sq, DEFAULTS, 1234);
      const b = impl.generate(sq, DEFAULTS, 1234);
      const c = impl.generate(sq, DEFAULTS, 99);
      expect(Array.from(a)).toEqual(Array.from(b));
      expect(Array.from(a)).not.toEqual(Array.from(c));
    });

    it('is well-formed', () => {
      const buf = impl.generate([50, 10, 90, 90, 10, 90], DEFAULTS, 1);
      expect(buf.length % OP_STRIDE).toBe(0);
      expect(buf[0]).toBe(OP_MOVE);
      for (let i = 0; i < buf.length; i += OP_STRIDE) expect([OP_MOVE, OP_BCURVE, OP_LINE]).toContain(buf[i]);
    });
  });
}
