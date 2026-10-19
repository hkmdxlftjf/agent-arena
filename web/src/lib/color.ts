export function shade(hex: string, factor: number): string {
  const full = hex.replace("#", "")
  const normalized = full.length === 3 ? full.split("").map((c) => c + c).join("") : full
  if (!/^[0-9a-fA-F]{6}$/.test(normalized)) return hex
  const num = Number.parseInt(normalized, 16)
  const channel = (shift: number) => Math.min(255, Math.round(((num >> shift) & 255) * factor))
  return `rgb(${channel(16)} ${channel(8)} ${channel(0)})`
}
