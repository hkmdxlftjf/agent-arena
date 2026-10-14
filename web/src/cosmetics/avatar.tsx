const FRAME: Record<string, string> = {
  cinnabar: "#8c3a2f",
  jade: "#1f6b56",
  ink: "#292524",
  gold: "#b8860b",
  season: "#6d28d9",
}

export function Avatar({
  preset = "crane",
  color = "#8c3a2f",
  frame,
  imageUrl,
  size = 40,
  title,
}: {
  preset?: string | null
  color?: string | null
  frame?: string | null
  imageUrl?: string | null
  size?: number
  title?: string | null
}) {
  const tone = color || "#8c3a2f"
  const ring = frame ? FRAME[frame] : null
  return (
    <span className="relative inline-grid shrink-0" style={{ width: size, height: size }} title={title || undefined}>
      {imageUrl ? (
        <img src={imageUrl} alt="" className="size-full rounded-full object-cover" />
      ) : (
        <svg viewBox="0 0 64 64" width={size} height={size} aria-hidden>
          <circle cx="32" cy="32" r="30" fill="#f6f1e8" />
          {glyph(preset || "crane", tone)}
        </svg>
      )}
      {ring && (
        <svg className="pointer-events-none absolute inset-0" viewBox="0 0 64 64" aria-hidden>
          <circle cx="32" cy="32" r="30" fill="none" stroke={ring} strokeWidth={frame === "gold" || frame === "season" ? 3.2 : 2.4} />
          {(frame === "gold" || frame === "season") && <circle cx="50" cy="14" r="4" fill={ring} />}
        </svg>
      )}
    </span>
  )
}

function glyph(preset: string, tone: string) {
  const ink = "#1c1612"
  const piece = preset.startsWith("ink-") ? preset.slice(4) : null
  if (piece) {
    const label = { general: "将", horse: "马", cannon: "炮", elephant: "象", soldier: "兵", chariot: "车" }[piece] || "子"
    return (
      <g>
        <circle cx="32" cy="32" r="18" fill="#fffaf3" stroke={ink} strokeWidth="2" />
        <text x="32" y="38" textAnchor="middle" fontSize="16" fontFamily="Noto Sans SC, sans-serif" fill={tone}>
          {label}
        </text>
      </g>
    )
  }
  if (preset === "robot-round" || preset === "robot-square") {
    const round = preset === "robot-round"
    return (
      <g>
        <line x1="32" y1="8" x2="32" y2="16" stroke={ink} strokeWidth="2" />
        <circle cx="32" cy="7" r="2.5" fill={tone} />
        {round ? (
          <circle cx="32" cy="34" r="16" fill={tone} />
        ) : (
          <rect x="16" y="18" width="32" height="30" rx="6" fill={tone} />
        )}
        <circle cx="26" cy="32" r="2.4" fill="#f6f1e8" />
        <circle cx="38" cy="32" r="2.4" fill="#f6f1e8" />
        <rect x="24" y="40" width="16" height="3" rx="1.5" fill="#f6f1e8" />
      </g>
    )
  }
  return animal(preset, tone, ink)
}

function animal(preset: string, tone: string, ink: string) {
  if (preset === "fox") {
    return (
      <g>
        <path d="M16 26 L24 14 L30 24" fill={tone} />
        <path d="M48 26 L40 14 L34 24" fill={tone} />
        <ellipse cx="32" cy="36" rx="16" ry="14" fill={tone} />
        <ellipse cx="32" cy="40" rx="7" ry="5" fill="#f6f1e8" />
        <circle cx="26" cy="34" r="1.8" fill={ink} />
        <circle cx="38" cy="34" r="1.8" fill={ink} />
      </g>
    )
  }
  if (preset === "panda") {
    return (
      <g>
        <circle cx="18" cy="20" r="7" fill={ink} />
        <circle cx="46" cy="20" r="7" fill={ink} />
        <circle cx="32" cy="34" r="16" fill="#f6f1e8" stroke={ink} />
        <ellipse cx="25" cy="33" rx="4" ry="5" fill={ink} />
        <ellipse cx="39" cy="33" rx="4" ry="5" fill={ink} />
        <ellipse cx="32" cy="41" rx="3" ry="2" fill={ink} />
      </g>
    )
  }
  if (preset === "koi") {
    return (
      <g>
        <path d="M10 32 C18 22 36 20 48 32 C36 44 18 42 10 32 Z" fill={tone} />
        <path d="M48 32 L58 22 L58 42 Z" fill={tone} />
        <circle cx="22" cy="30" r="1.6" fill={ink} />
        <circle cx="34" cy="28" r="3" fill="#f6f1e8" opacity="0.8" />
      </g>
    )
  }
  if (preset === "rabbit") {
    return (
      <g>
        <ellipse cx="24" cy="16" rx="4" ry="10" fill={tone} />
        <ellipse cx="40" cy="16" rx="4" ry="10" fill={tone} />
        <circle cx="32" cy="38" r="14" fill={tone} />
        <circle cx="27" cy="36" r="1.6" fill={ink} />
        <circle cx="37" cy="36" r="1.6" fill={ink} />
      </g>
    )
  }
  if (preset === "tiger") {
    return (
      <g>
        <path d="M18 24 L22 14 L28 24" fill={tone} />
        <path d="M46 24 L42 14 L36 24" fill={tone} />
        <circle cx="32" cy="36" r="15" fill={tone} />
        <path d="M22 30 H28 M36 30 H42 M24 40 H40" stroke={ink} strokeWidth="1.6" />
        <circle cx="26" cy="34" r="1.6" fill={ink} />
        <circle cx="38" cy="34" r="1.6" fill={ink} />
      </g>
    )
  }
  if (preset === "owl") {
    return (
      <g>
        <path d="M18 22 L22 12 L28 22" fill={tone} />
        <path d="M46 22 L42 12 L36 22" fill={tone} />
        <ellipse cx="32" cy="36" rx="16" ry="15" fill={tone} />
        <circle cx="25" cy="34" r="6" fill="#f6f1e8" />
        <circle cx="39" cy="34" r="6" fill="#f6f1e8" />
        <circle cx="25" cy="34" r="2.2" fill={ink} />
        <circle cx="39" cy="34" r="2.2" fill={ink} />
      </g>
    )
  }
  if (preset === "turtle") {
    return (
      <g>
        <ellipse cx="32" cy="36" rx="16" ry="12" fill={tone} />
        <path d="M20 30 H44 M24 36 H40 M22 42 H42" stroke="#f6f1e8" strokeWidth="1.4" />
        <circle cx="48" cy="30" r="5" fill={tone} />
        <circle cx="50" cy="29" r="1" fill={ink} />
      </g>
    )
  }
  if (preset === "cat") {
    return (
      <g>
        <path d="M18 28 L22 14 L30 26" fill={tone} />
        <path d="M46 28 L42 14 L34 26" fill={tone} />
        <circle cx="32" cy="36" r="14" fill={tone} />
        <circle cx="26" cy="34" r="1.7" fill="#f6f1e8" />
        <circle cx="38" cy="34" r="1.7" fill="#f6f1e8" />
        <path d="M16 38 H24 M40 38 H48" stroke={ink} strokeWidth="1.2" />
      </g>
    )
  }
  if (preset === "deer") {
    return (
      <g>
        <path d="M22 28 L18 12 M22 20 L14 16 M42 28 L46 12 M42 20 L50 16" stroke={tone} strokeWidth="2" fill="none" />
        <ellipse cx="32" cy="40" rx="12" ry="11" fill={tone} />
        <circle cx="27" cy="38" r="1.5" fill={ink} />
        <circle cx="37" cy="38" r="1.5" fill={ink} />
      </g>
    )
  }
  return (
    <g>
      <path d="M20 40 C22 22 28 16 32 12 C36 16 42 22 44 40" fill={tone} />
      <path d="M24 42 H40" stroke={ink} strokeWidth="2" />
      <circle cx="30" cy="28" r="1.4" fill={ink} />
    </g>
  )
}
