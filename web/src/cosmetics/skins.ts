export type StonePaint = { fill: string; stroke: string; mark: string }

const PAPER = { fill: "#f3e6c8", line: "#8a7b6a", label: "#6b6258", river: "#f3d7b5", edge: "#c4a574" }

export function boardPaint(theme?: string | null) {
  if (theme === "celadon") {
    return { fill: "#d7ebe3", line: "#3d6b5e", label: "#245248", river: "#c5e0d6", edge: "#8fb8aa" }
  }
  if (theme === "night") {
    return { fill: "#1c1917", line: "#a8a29e", label: "#d6d3d1", river: "#292524", edge: "#44403c" }
  }
  if (theme === "wood") {
    return { fill: "#c4a574", line: "#5c3d24", label: "#3f2a1b", river: "#d7b98a", edge: "#8a6844" }
  }
  return null
}

export function paperBoard() {
  return PAPER
}

export function gomokuStone(asset: string | null | undefined, seat: number, theme?: string | null): StonePaint {
  const dark = seat === 0
  const tone = (() => {
    if (asset === "jade") {
      return dark
        ? { fill: "#0e3b30", stroke: "#0e3b30", mark: "#d7f5e8" }
        : { fill: "#e7f6ef", stroke: "#1f6b56", mark: "#0e3b30" }
    }
    if (asset === "ink") {
      return dark
        ? { fill: "#1c1917", stroke: "#1c1917", mark: "#f5f5f4" }
        : { fill: "#f4efe6", stroke: "#78716c", mark: "#1c1917" }
    }
    if (asset === "wood") {
      return dark
        ? { fill: "#3f2a1d", stroke: "#3f2a1d", mark: "#f3e0c4" }
        : { fill: "#f3e0c4", stroke: "#6b4a2e", mark: "#3f2a1d" }
    }
    return dark
      ? { fill: "#111111", stroke: "#111111", mark: "#ffffff" }
      : { fill: "#ffffff", stroke: "#d4d4d8", mark: "var(--primary)" }
  })()
  if (theme === "night" && dark) return { ...tone, stroke: "#e7e5e4" }
  return tone
}

export function xiangqiDisc(asset: string | null | undefined, red: boolean) {
  if (asset === "porcelain") {
    return { fill: "#f7fbff", stroke: red ? "#b42318" : "#334155" }
  }
  return { fill: "#fffaf3", stroke: red ? "#b42318" : "#1c1917" }
}

export function cardBackClass(asset?: string | null) {
  if (asset === "brocade") return "border-[#7a2e24] bg-[repeating-linear-gradient(135deg,#8c3a2f_0_6px,#e7d3b0_6px_9px)]"
  if (asset === "ink") return "border-[#44403c] bg-[#f4efe6] shadow-[inset_0_0_0_3px_#1c1917]"
  return "border-[#243038] bg-gradient-to-br from-[#3d4c55] to-[#243038]"
}

export function tileBackClass(asset?: string | null) {
  if (asset === "brocade") return "border-[#7a2e24] bg-[repeating-linear-gradient(135deg,#8c3a2f_0_4px,#e7d3b0_4px_6px)]"
  if (asset === "ink") return "border-[#44403c] bg-[#f4efe6]"
  return "border-[#1c3a32] bg-[#2f6b57]"
}
