- [x] rectangle
- [x] ellipse
- [x] rectangle test (for both original and new)
- [x] ellipse test (for both original and new)
- [ ] polygon test
- [ ] line test

- [ ] better  interface
export type ShapeImpl =
  | {
      name: "ellipse";
      generate(
        x: number,
        y: number,
        w: number,
        h: number,
        o: FlatOptions,
        seed: number,
      ): EllipseOp;
    }
  | {
      name: "rectangle";
      generate(
        x: number,
        y: number,
        w: number,
        h: number,
        o: FlatOptions,
        seed: number,
      ): RectangleOp;
    };

    const EllipseOp = {
      optionX: 0,
optionY: 1
    }
    // same thing for rectangle op