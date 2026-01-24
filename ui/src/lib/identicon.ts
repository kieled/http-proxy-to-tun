import * as jdenticon from 'jdenticon'

export function generateIdenticon(value: string, size = 32): string {
  return jdenticon.toSvg(value, size)
}

export function getIdenticonDataUrl(value: string, size = 32): string {
  const svg = generateIdenticon(value, size)
  const encoded = btoa(svg)
  return `data:image/svg+xml;base64,${encoded}`
}
