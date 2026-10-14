const FILES = "abcdefghijklmnopqrs"

export function GridBoard({
  size,
  cells,
  compact = false,
}: {
  size: number
  cells: { at: string; seat?: number; label?: string }[]
  compact?: boolean
}) {
  const n = Math.max(1, Math.min(size || 3, 19))
  const files = FILES.slice(0, n)
  const marks = new Map(cells.map((cell) => [cell.at, cell]))
  const box = compact ? "h-7 w-7 text-xs" : "h-16 w-16 text-2xl sm:h-[4.5rem] sm:w-[4.5rem]"
  return (
    <div className="paper-board inline-block rounded-2xl border border-[#c4a574] p-3">
      <div className="grid gap-px" style={{ gridTemplateColumns: `repeat(${n}, minmax(0, 1fr))` }}>
        {Array.from({ length: n }, (_, row) => {
          const rank = n - row
          return files.split("").map((file) => {
            const at = `${file}${rank}`
            const mark = marks.get(at)
            const tone = mark?.seat === 0 ? "text-primary" : "text-neutral-800"
            return (
              <div key={at} className={`${box} grid place-items-center border border-[#e0d3c2] bg-card font-medium ${tone}`}>
                {mark?.label ?? ""}
              </div>
            )
          })
        })}
      </div>
    </div>
  )
}
