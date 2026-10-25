export function offsets(heights: number[]): number[] {
  const out = [0]
  for (const h of heights) out.push(out[out.length - 1] + h)
  return out
}

export function visibleRange(offs: number[], top: number, height: number, overscan = 3): [number, number] {
  const find = (y: number) => {
    let lo = 0, hi = offs.length - 2
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (offs[mid] <= y) lo = mid
      else hi = mid - 1
    }
    return lo
  }
  const n = offs.length - 1
  if (n <= 0) return [0, 0]
  return [Math.max(0, find(top) - overscan), Math.min(n, find(top + height) + 1 + overscan)]
}
