import { describe, it, expect } from 'vitest';
import {
  lineImpls,
  referenceLine,
  DEFAULTS,
  FlatOptions,
  OP_STRIDE,
  OP_MOVE,
  OP_BCURVE,
  OP_LINE,
} from './fidelity-impls.js';

interface Case {
  name: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  seed: number; // non-zero
  o?: Partial<FlatOptions>;
}

const cases: Case[] = [
  { name: 'short (len <200)', x1: 10, y1: 10, x2: 90, y2: 90, seed: 1 },
  { name: 'long (>500 gain branch)', x1: 0, y1: 0, x2: 700, y2: 100, seed: 42 },
  { name: 'mid (200-500 gain branch)', x1: 0, y1: 0, x2: 300, y2: 0, seed: 7 },
  { name: 'horizontal', x1: 20, y1: 50, x2: 220, y2: 50, seed: 9 },
  { name: 'vertical', x1: 50, y1: 20, x2: 50, y2: 260, seed: 11 },
  { name: 'high roughness', x1: 10, y1: 10, x2: 120, y2: 80, seed: 5, o: { roughness: 3 } },
  { name: 'preserveVertices', x1: 10, y1: 10, x2: 120, y2: 80, seed: 5, o: { preserveVertices: true } },
  { name: 'single stroke', x1: 10, y1: 10, x2: 120, y2: 80, seed: 5, o: { disableMultiStroke: true } },
  { name: 'no bowing', x1: 10, y1: 10, x2: 120, y2: 80, seed: 5, o: { bowing: 0 } },
];

const EPS = 1e-9; // no trig on the line path

const optsFor = (c: Case): FlatOptions => ({ ...DEFAULTS, ...c.o });

for (const impl of lineImpls) {
  describe(`line fidelity: ${impl.name}`, () => {
    for (const c of cases) {
      it(`matches rough.js — ${c.name}`, () => {
        const o = optsFor(c);
        const actual = impl.generate(c.x1, c.y1, c.x2, c.y2, o, c.seed);
        const expected = referenceLine(c.x1, c.y1, c.x2, c.y2, o, c.seed);
        expect(actual.length).toBe(expected.length);
        for (let i = 0; i < expected.length; i++) {
          if (i % OP_STRIDE === 0) expect(actual[i]).toBe(expected[i]);
          else expect(Math.abs(actual[i] - expected[i])).toBeLessThanOrEqual(EPS * Math.max(1, Math.abs(expected[i])));
        }
      });
    }

    it('emits 4 ops (2 strokes x move+bcurve)', () => {
      expect(impl.generate(10, 10, 90, 90, DEFAULTS, 1).length).toBe(4 * OP_STRIDE);
    });

    it('emits 2 ops when multi-stroke is disabled', () => {
      expect(impl.generate(10, 10, 90, 90, { ...DEFAULTS, disableMultiStroke: true }, 1).length).toBe(2 * OP_STRIDE);
    });

    it('is deterministic and seed-sensitive', () => {
      const a = impl.generate(5, 6, 77, 88, DEFAULTS, 1234);
      const b = impl.generate(5, 6, 77, 88, DEFAULTS, 1234);
      const c = impl.generate(5, 6, 77, 88, DEFAULTS, 99);
      expect(Array.from(a)).toEqual(Array.from(b));
      expect(Array.from(a)).not.toEqual(Array.from(c));
    });

    it('is well-formed', () => {
      const buf = impl.generate(10, 10, 90, 90, DEFAULTS, 1);
      expect(buf.length % OP_STRIDE).toBe(0);
      expect(buf[0]).toBe(OP_MOVE);
      for (let i = 0; i < buf.length; i += OP_STRIDE) expect([OP_MOVE, OP_BCURVE, OP_LINE]).toContain(buf[i]);
    });
  });
}
