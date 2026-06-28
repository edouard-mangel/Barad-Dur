// jsdom does not implement SVG layout APIs that d3 may touch during rendering.
// Provide a no-op getBBox so components using d3 (e.g. HotspotsView) can render
// in tests without throwing.
if (typeof SVGElement !== 'undefined' && !('getBBox' in SVGElement.prototype)) {
  // @ts-expect-error – jsdom shim, signature intentionally minimal
  SVGElement.prototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 })
}
