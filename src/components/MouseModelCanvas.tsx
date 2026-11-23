// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { useEffect, useRef } from 'react';
import { fitCanvas } from '../lib/draw';
import { drawMouse, idleMouse, mouseHeight } from '../models/drawMouse';
import type { ShapeId } from '../models/mouseShapes';

/** a still mouse silhouette, for the picker and the device file */
export function MouseModelCanvas({ shape, width = 120, label }: { shape: ShapeId; width?: number; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const h = mouseHeight(width);
    const { ctx } = fitCanvas(ref.current!, width, h);
    ctx.clearRect(0, 0, width, h);
    drawMouse(ctx, shape, 0, 0, width, idleMouse(), false);
  }, [shape, width]);
  return <canvas ref={ref} role="img" aria-label={label} />;
}
