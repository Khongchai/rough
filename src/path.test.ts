import { describe, it, expect } from 'vitest';
import {
  pathImpls,
  referencePath,
  DEFAULTS,
  FlatOptions,
  OP_STRIDE,
  OP_MOVE,
  OP_BCURVE,
  OP_LINE,
} from './fidelity-impls.js';

interface Case {
  name: string;
  d: string;
  seed: number; // non-zero
  o?: Partial<FlatOptions>;
}

// Cover the segment kinds normalize() handles (M/L/H/V/C/S/Q/T/Z) and relative variants.
// Arcs (A) use trig in arc->cubic so we leave them out of the exact-match set.
const cases: Case[] = [
  { name: 'M L Z triangle', d: 'M10 10 L90 10 L50 90 Z', seed: 1 },
  { name: 'relative l', d: 'M10 10 l80 0 l-40 80 z', seed: 2 },
  { name: 'H and V', d: 'M20 20 H120 V120 H20 Z', seed: 3 },
  { name: 'cubic C', d: 'M10 80 C40 10 65 10 95 80 S150 150 180 80', seed: 4 },
  { name: 'quadratic Q/T', d: 'M10 80 Q52.5 10 95 80 T180 80', seed: 5 },
  { name: 'multiple subpaths', d: 'M10 10 L60 10 M10 60 L60 60', seed: 6 },
  { name: 'implicit lineto', d: 'M10 10 40 40 70 10', seed: 7 },
  { name: 'high roughness', d: 'M10 10 L90 10 L50 90 Z', seed: 8, o: { roughness: 3 } },
  { name: 'preserveVertices', d: 'M10 80 C40 10 65 10 95 80', seed: 9, o: { preserveVertices: true } },
  { name: 'single stroke', d: 'M10 10 L90 10 L50 90 Z', seed: 10, o: { disableMultiStroke: true } },
];

const EPS = 1e-9; // M/L/C/Q/S/T/H/V/Z normalize with arithmetic only (no trig)

const optsFor = (c: Case): FlatOptions => ({ ...DEFAULTS, ...c.o });

for (const impl of pathImpls) {
  describe(`path fidelity: ${impl.name}`, () => {
    for (const c of cases) {
      it(`matches rough.js — ${c.name}`, () => {
        const o = optsFor(c);
        const actual = impl.generate(c.d, o, c.seed);
        const expected = referencePath(c.d, o, c.seed);
        expect(actual.length).toBe(expected.length);
        for (let i = 0; i < expected.length; i++) {
          if (i % OP_STRIDE === 0) expect(actual[i]).toBe(expected[i]);
          else expect(Math.abs(actual[i] - expected[i])).toBeLessThanOrEqual(EPS * Math.max(1, Math.abs(expected[i])));
        }
      });
    }

    it('empty path produces no ops', () => {
      expect(impl.generate('', DEFAULTS, 1).length).toBe(0);
    });

    it('is deterministic and seed-sensitive', () => {
      const d = 'M10 80 C40 10 65 10 95 80 S150 150 180 80';
      const a = impl.generate(d, DEFAULTS, 1234);
      const b = impl.generate(d, DEFAULTS, 1234);
      const c = impl.generate(d, DEFAULTS, 99);
      expect(Array.from(a)).toEqual(Array.from(b));
      expect(Array.from(a)).not.toEqual(Array.from(c));
    });

    it('is well-formed', () => {
      const buf = impl.generate('M10 10 L90 10 L50 90 Z', DEFAULTS, 1);
      expect(buf.length).toBeGreaterThan(0);
      expect(buf.length % OP_STRIDE).toBe(0);
      expect(buf[0]).toBe(OP_MOVE);
      for (let i = 0; i < buf.length; i += OP_STRIDE) expect([OP_MOVE, OP_BCURVE, OP_LINE]).toContain(buf[i]);
    });
  });
}
