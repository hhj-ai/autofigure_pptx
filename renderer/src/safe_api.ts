export type NormalizedBox = [number, number, number, number];

export interface LayoutObject {
  id: string;
  kind: "component" | "edge" | "annotation" | "region" | "label" | "image";
  bbox: NormalizedBox;
  points?: Array<[number, number]>;
}

export interface LayoutMap {
  canvas: {
    width: number;
    height: number;
    aspect: string;
    target_width_mm: number;
  };
  objects: LayoutObject[];
}

export function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

export function normalizeBox(box: NormalizedBox): NormalizedBox {
  const [x1, y1, x2, y2] = box.map(clamp01) as NormalizedBox;
  return [Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2)];
}
