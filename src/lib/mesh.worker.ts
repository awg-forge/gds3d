import earcut from "earcut";

type PolygonInput = {
  points: number[][];
  holes?: number[][][];
};

type LayerInput = {
  id: string;
  depth: number;
  polygons: PolygonInput[];
};

type MeshRequest = {
  layers: LayerInput[];
};

type LayerMeshData = {
  id: string;
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  triangleRanges: TriangleRange[];
};

type TriangleRange = {
  startFaceId: number;
  endFaceId: number;
  polygonIndex: number;
};

type MeshResponse = { ok: true; layers: LayerMeshData[] } | { ok: false; message: string };

type WorkerScope = {
  addEventListener: (type: "message", listener: (event: MessageEvent<MeshRequest>) => void) => void;
  postMessage: (message: MeshResponse, transfer?: Transferable[]) => void;
};

const worker = self as unknown as WorkerScope;

worker.addEventListener("message", ({ data }) => {
  try {
    const layers = data.layers.map(buildLayerMesh);
    const transfer: ArrayBuffer[] = layers.flatMap(({ positions, normals, indices }) => [
      positions.buffer as ArrayBuffer,
      normals.buffer as ArrayBuffer,
      indices.buffer as ArrayBuffer,
    ]);
    // oxlint-disable-next-line unicorn/require-post-message-target-origin -- Worker.postMessage has no targetOrigin parameter.
    worker.postMessage({ ok: true, layers }, transfer);
  } catch (reason) {
    const response: MeshResponse = {
      ok: false,
      message: reason instanceof Error ? reason.message : String(reason),
    };
    // oxlint-disable-next-line unicorn/require-post-message-target-origin -- Worker.postMessage has no targetOrigin parameter.
    worker.postMessage(response);
  }
});

function buildLayerMesh(layer: LayerInput): LayerMeshData {
  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];
  const triangleRanges: TriangleRange[] = [];

  for (const [polygonIndex, polygon] of layer.polygons.entries()) {
    const triangleRange = appendPolygon(positions, normals, indices, polygon, layer.depth);
    if (triangleRange) triangleRanges.push({ ...triangleRange, polygonIndex });
  }

  return {
    id: layer.id,
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    triangleRanges,
  };
}

function appendPolygon(
  positions: number[],
  normals: number[],
  indices: number[],
  polygon: PolygonInput,
  depth: number,
): Omit<TriangleRange, "polygonIndex"> | null {
  const contours = [polygon.points, ...(polygon.holes ?? [])].filter(
    (contour) => contour.length >= 3,
  );
  if (contours.length === 0) return null;

  const coordinates: number[] = [];
  const holeIndices: number[] = [];
  let vertexCount = 0;
  for (const [contourIndex, contour] of contours.entries()) {
    if (contourIndex > 0) holeIndices.push(vertexCount);
    for (const point of contour) {
      const x = point[0];
      const y = point[1];
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        throw new Error("polygon contains a non-finite coordinate");
      }
      coordinates.push(x, y);
      vertexCount += 1;
    }
  }

  const triangles = earcut(coordinates, holeIndices, 2);
  if (triangles.length === 0) return null;

  const startFaceId = indices.length / 3;

  const topOffset = positions.length / 3;
  for (let index = 0; index < coordinates.length; index += 2) {
    positions.push(coordinates[index], 0, coordinates[index + 1]);
    normals.push(0, 1, 0);
  }
  const bottomOffset = positions.length / 3;
  for (let index = 0; index < coordinates.length; index += 2) {
    positions.push(coordinates[index], -depth, coordinates[index + 1]);
    normals.push(0, -1, 0);
  }

  for (let index = 0; index < triangles.length; index += 3) {
    const a = triangles[index];
    const b = triangles[index + 1];
    const c = triangles[index + 2];
    indices.push(topOffset + a, topOffset + b, topOffset + c);
    indices.push(bottomOffset + c, bottomOffset + b, bottomOffset + a);
  }

  for (const [contourIndex, contour] of contours.entries()) {
    const hole = contourIndex > 0;
    for (let index = 0; index < contour.length; index += 1) {
      const [x1, y1] = contour[index];
      const [x2, y2] = contour[(index + 1) % contour.length];
      const dx = x2 - x1;
      const dy = y2 - y1;
      const length = Math.hypot(dx, dy);
      if (length === 0) continue;
      const direction = hole ? -1 : 1;
      const nx = (direction * dy) / length;
      const nz = (-direction * dx) / length;
      const sideOffset = positions.length / 3;
      positions.push(x1, 0, y1, x1, -depth, y1, x2, 0, y2, x2, -depth, y2);
      for (let vertex = 0; vertex < 4; vertex += 1) normals.push(nx, 0, nz);
      if (hole) {
        indices.push(
          sideOffset,
          sideOffset + 2,
          sideOffset + 1,
          sideOffset + 1,
          sideOffset + 2,
          sideOffset + 3,
        );
      } else {
        indices.push(
          sideOffset,
          sideOffset + 1,
          sideOffset + 2,
          sideOffset + 1,
          sideOffset + 3,
          sideOffset + 2,
        );
      }
    }
  }

  return { startFaceId, endFaceId: indices.length / 3 };
}
