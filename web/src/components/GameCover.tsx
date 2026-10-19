import { gameTone } from "@/lib/games"
import { useId, type ReactNode } from "react"

const SERIF = '"Noto Serif SC", "Songti SC", "STSong", "SimSun", serif'

type Ids = { bg: string; glow: string; black: string; white: string; wood: string; shade: string }

function Stone({ x, y, r, black, ids }: { x: number; y: number; r: number; black: boolean; ids: Ids }) {
  return (
    <g>
      <circle cx={x + r * 0.18} cy={y + r * 0.28} r={r} fill="rgb(20 12 4 / 35%)" />
      <circle cx={x} cy={y} r={r} fill={`url(#${black ? ids.black : ids.white})`} />
    </g>
  )
}

function Grid({ x, y, w, h, step, stroke }: { x: number; y: number; w: number; h: number; step: number; stroke: string }) {
  const lines: ReactNode[] = []
  for (let i = 0; i * step <= w; i++) lines.push(<line key={`v${i}`} x1={x + i * step} y1={y} x2={x + i * step} y2={y + h} stroke={stroke} strokeWidth="1" />)
  for (let i = 0; i * step <= h; i++) lines.push(<line key={`h${i}`} x1={x} y1={y + i * step} x2={x + w} y2={y + i * step} stroke={stroke} strokeWidth="1" />)
  return <g>{lines}</g>
}

function Gomoku({ ids, tone }: { ids: Ids; tone: string }) {
  const s = 20
  const at = (c: number, r: number) => ({ x: 138 + c * s, y: 18 + r * s })
  const black = [[2, 6], [3, 5], [4, 4], [5, 3], [6, 2], [4, 6], [7, 5]]
  const white = [[3, 4], [4, 5], [5, 5], [6, 4], [3, 6], [5, 6]]
  return (
    <>
      <rect x="120" y="0" width="220" height="190" fill={`url(#${ids.wood})`} />
      <Grid x={138} y={18} w={180} h={160} step={s} stroke="rgb(58 34 14 / 55%)" />
      <line x1={at(2, 6).x} y1={at(2, 6).y} x2={at(6, 2).x} y2={at(6, 2).y} stroke={tone} strokeWidth="14" strokeLinecap="round" opacity="0.28" />
      {white.map(([c, r]) => <Stone key={`w${c}${r}`} {...at(c, r)} r={8.5} black={false} ids={ids} />)}
      {black.map(([c, r]) => <Stone key={`b${c}${r}`} {...at(c, r)} r={8.5} black ids={ids} />)}
      <rect x="120" y="0" width="220" height="190" fill={`url(#${ids.shade})`} />
    </>
  )
}

function Go({ ids }: { ids: Ids }) {
  const s = 15
  const at = (c: number, r: number) => ({ x: 128 + c * s, y: 14 + r * s })
  const black = [[3, 3], [4, 3], [5, 4], [4, 5], [6, 5], [3, 6], [7, 6], [8, 4], [9, 8], [5, 8], [6, 9]]
  const white = [[5, 3], [6, 4], [5, 5], [5, 6], [6, 7], [7, 5], [8, 7], [4, 7], [7, 8], [10, 6]]
  return (
    <>
      <rect x="112" y="0" width="230" height="190" fill={`url(#${ids.wood})`} />
      <Grid x={128} y={14} w={210} h={165} step={s} stroke="rgb(58 34 14 / 50%)" />
      {[[3, 3], [9, 3], [3, 9], [9, 9], [6, 6]].map(([c, r]) => (
        <circle key={`h${c}${r}`} cx={at(c, r).x} cy={at(c, r).y} r="2" fill="rgb(58 34 14 / 70%)" />
      ))}
      {white.map(([c, r]) => <Stone key={`w${c}${r}`} {...at(c, r)} r={6.6} black={false} ids={ids} />)}
      {black.map(([c, r]) => <Stone key={`b${c}${r}`} {...at(c, r)} r={6.6} black ids={ids} />)}
      <rect x="112" y="0" width="230" height="190" fill={`url(#${ids.shade})`} />
    </>
  )
}

function Disc({ x, y, label, red }: { x: number; y: number; label: string; red: boolean }) {
  const ink = red ? "#b42318" : "#1c1917"
  return (
    <g>
      <circle cx={x + 2} cy={y + 3} r="17" fill="rgb(20 12 4 / 35%)" />
      <circle cx={x} cy={y} r="17" fill="#f6e7c8" stroke="#c9a26a" strokeWidth="1.5" />
      <circle cx={x} cy={y} r="13.5" fill="none" stroke={ink} strokeWidth="1.3" />
      <text x={x} y={y + 6} textAnchor="middle" fontSize="16" fontWeight="700" fill={ink} fontFamily={SERIF}>
        {label}
      </text>
    </g>
  )
}

function Xiangqi({ ids }: { ids: Ids }) {
  return (
    <>
      <rect x="118" y="0" width="230" height="190" fill={`url(#${ids.wood})`} />
      <Grid x={136} y={-10} w={192} h={72} step={24} stroke="rgb(58 34 14 / 55%)" />
      <Grid x={136} y={110} w={192} h={96} step={24} stroke="rgb(58 34 14 / 55%)" />
      <rect x="136" y="62" width="192" height="48" fill="rgb(58 34 14 / 10%)" />
      <text x="188" y="92" textAnchor="middle" fontSize="15" fill="rgb(58 34 14 / 72%)" fontFamily={SERIF} letterSpacing="6">楚河</text>
      <text x="280" y="92" textAnchor="middle" fontSize="15" fill="rgb(58 34 14 / 72%)" fontFamily={SERIF} letterSpacing="6">漢界</text>
      <Disc x={184} y={38} label="將" red={false} />
      <Disc x={256} y={14} label="馬" red={false} />
      <Disc x={232} y={134} label="帥" red />
      <Disc x={160} y={158} label="炮" red />
      <Disc x={304} y={134} label="車" red />
      <rect x="118" y="0" width="230" height="190" fill={`url(#${ids.shade})`} />
    </>
  )
}

function JunqiPiece({ x, y, label, red, rot = 0 }: { x: number; y: number; label: string; red: boolean; rot?: number }) {
  return (
    <g transform={`rotate(${rot} ${x} ${y})`}>
      <rect x={x - 19} y={y - 9} width="42" height="24" rx="4" fill="rgb(10 14 20 / 35%)" />
      <rect x={x - 21} y={y - 12} width="42" height="24" rx="4" fill={red ? "#9f1239" : "#1e3a5f"} stroke="rgb(255 255 255 / 30%)" />
      <text x={x} y={y + 5} textAnchor="middle" fontSize="13" fontWeight="700" fill="#fffaf3" fontFamily={SERIF}>
        {label}
      </text>
    </g>
  )
}

function Junqi({ ids }: { ids: Ids }) {
  return (
    <>
      <rect x="118" y="0" width="230" height="190" fill="#cfe0bd" />
      <Grid x={140} y={10} w={180} h={170} step={36} stroke="rgb(40 60 30 / 45%)" />
      {[[176, 46], [248, 46], [212, 82], [176, 118], [248, 118]].map(([cx, cy]) => (
        <circle key={`${cx}${cy}`} cx={cx} cy={cy} r="13" fill="#cfe0bd" stroke="rgb(40 60 30 / 55%)" />
      ))}
      <JunqiPiece x={176} y={46} label="司令" red rot={-6} />
      <JunqiPiece x={248} y={118} label="军长" red={false} rot={4} />
      <JunqiPiece x={284} y={46} label="炸弹" red rot={3} />
      <JunqiPiece x={176} y={154} label="工兵" red={false} rot={-3} />
      <g transform="translate(298 128)">
        <line x1="0" y1="0" x2="0" y2="44" stroke="#3b2a1a" strokeWidth="2.5" />
        <path d="M1 2 L28 9 L1 18 Z" fill="#dc2626" />
      </g>
      <rect x="118" y="0" width="230" height="190" fill={`url(#${ids.shade})`} />
    </>
  )
}

function Card({ x, y, rot, rank, suit, red }: { x: number; y: number; rot: number; rank: string; suit: string; red: boolean }) {
  const ink = red ? "#c81e1e" : "#111827"
  return (
    <g transform={`rotate(${rot} ${x + 34} ${y + 96})`}>
      <rect x={x + 3} y={y + 5} width="68" height="96" rx="7" fill="rgb(0 0 0 / 35%)" />
      <rect x={x} y={y} width="68" height="96" rx="7" fill="#fbfaf7" stroke="#e5e1d8" />
      <text x={x + 9} y={y + 21} fontSize="16" fontWeight="700" fill={ink} fontFamily={SERIF}>{rank}</text>
      <text x={x + 34} y={y + 64} textAnchor="middle" fontSize="30" fill={ink} fontFamily={SERIF}>{suit}</text>
    </g>
  )
}

function Doudizhu({ tone }: { tone: string }) {
  return (
    <>
      <ellipse cx="236" cy="168" rx="120" ry="34" fill={tone} opacity="0.14" />
      <Card x={156} y={46} rot={-18} rank="3" suit="♣" red={false} />
      <Card x={182} y={34} rot={-6} rank="K" suit="♥" red />
      <Card x={208} y={30} rot={6} rank="A" suit="♠" red={false} />
      <g transform="rotate(18 268 126)">
        <rect x={237} y={35} width="68" height="96" rx="7" fill="rgb(0 0 0 / 35%)" />
        <rect x={234} y={30} width="68" height="96" rx="7" fill="#fbfaf7" stroke="#e5e1d8" />
        <text x={243} y={50} fontSize="11" fontWeight="700" fill="#c81e1e" fontFamily={SERIF}>大</text>
        <text x={243} y={63} fontSize="11" fontWeight="700" fill="#c81e1e" fontFamily={SERIF}>王</text>
        <path d="M250 100 l8 -22 l8 14 l8 -18 l8 18 l8 -14 l-2 22 z" fill="#f59e0b" stroke="#b45309" strokeWidth="1" />
      </g>
    </>
  )
}

function Tile({ x, y, children }: { x: number; y: number; children: ReactNode }) {
  return (
    <g>
      <rect x={x + 2} y={y + 6} width="44" height="60" rx="6" fill="#1f5d4b" />
      <rect x={x} y={y} width="44" height="60" rx="6" fill="#f8f4ea" stroke="#d9d1bf" />
      <g transform={`translate(${x} ${y})`}>{children}</g>
    </g>
  )
}

function Mahjong({ tone }: { tone: string }) {
  return (
    <>
      <ellipse cx="236" cy="150" rx="118" ry="30" fill={tone} opacity="0.14" />
      <g transform="rotate(10 146 44)">
        <rect x="128" y="20" width="36" height="48" rx="5" fill="#2f6b57" stroke="#1f5d4b" />
      </g>
      <g transform="rotate(-6 236 90)">
        <Tile x={140} y={62}>
          <text x="22" y="40" textAnchor="middle" fontSize="28" fontWeight="700" fill="#c81e1e" fontFamily={SERIF}>中</text>
        </Tile>
        <Tile x={190} y={62}>
          <text x="22" y="40" textAnchor="middle" fontSize="26" fontWeight="700" fill="#15803d" fontFamily={SERIF}>發</text>
        </Tile>
        <Tile x={240} y={62}>
          {[[14, 16], [30, 16], [22, 30], [14, 44], [30, 44]].map(([cx, cy]) => (
            <g key={`${cx}${cy}`}>
              <circle cx={cx} cy={cy} r="6" fill="none" stroke="#1d4ed8" strokeWidth="2" />
              <circle cx={cx} cy={cy} r="2" fill="#c81e1e" />
            </g>
          ))}
        </Tile>
        <Tile x={290} y={62}>
          {[12, 22, 32].map((bx) => (
            <g key={bx}>
              <rect x={bx - 2.5} y="10" width="5" height="18" rx="2" fill="#15803d" />
              <rect x={bx - 2.5} y="32" width="5" height="18" rx="2" fill="#15803d" />
            </g>
          ))}
        </Tile>
      </g>
    </>
  )
}

function Uploaded({ tone, initial }: { tone: string; initial: string }) {
  return (
    <>
      <Grid x={150} y={20} w={170} h={150} step={30} stroke="rgb(255 255 255 / 9%)" />
      <text x="196" y="122" textAnchor="middle" fontSize="88" fontWeight="300" fill={tone} opacity="0.55" fontFamily="ui-monospace, monospace">{"{"}</text>
      <text x="294" y="122" textAnchor="middle" fontSize="88" fontWeight="300" fill={tone} opacity="0.55" fontFamily="ui-monospace, monospace">{"}"}</text>
      <text x="245" y="112" textAnchor="middle" fontSize="44" fontWeight="700" fill="#f2f5fa" fontFamily={SERIF}>{initial}</text>
    </>
  )
}

/** Original vector cover art per built-in game; uploaded games get a generic code cover. */
export function GameCover({ id, name = "", className = "" }: { id: string; name?: string; className?: string }) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "")
  const ids: Ids = {
    bg: `${uid}bg`,
    glow: `${uid}glow`,
    black: `${uid}black`,
    white: `${uid}white`,
    wood: `${uid}wood`,
    shade: `${uid}shade`,
  }
  const tone = gameTone(id)
  let art: ReactNode
  if (id === "gomoku") art = <Gomoku ids={ids} tone={tone} />
  else if (id === "go") art = <Go ids={ids} />
  else if (id === "xiangqi") art = <Xiangqi ids={ids} />
  else if (id === "junqi") art = <Junqi ids={ids} />
  else if (id === "doudizhu") art = <Doudizhu tone={tone} />
  else if (id === "mahjong-sichuan") art = <Mahjong tone={tone} />
  else art = <Uploaded tone={tone} initial={name.trim().slice(0, 1) || "?"} />

  return (
    <svg viewBox="0 0 320 180" preserveAspectRatio="xMidYMid slice" className={`block h-full w-full ${className}`} aria-hidden="true">
      <defs>
        <linearGradient id={ids.bg} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#0b1019" />
          <stop offset="1" stopColor={tone} stopOpacity="0.28" />
        </linearGradient>
        <radialGradient id={ids.glow} cx="0.2" cy="0.2" r="0.7">
          <stop offset="0" stopColor={tone} stopOpacity="0.32" />
          <stop offset="1" stopColor={tone} stopOpacity="0" />
        </radialGradient>
        <radialGradient id={ids.black} cx="0.35" cy="0.3" r="0.75">
          <stop offset="0" stopColor="#5b6170" />
          <stop offset="0.45" stopColor="#1b1e24" />
          <stop offset="1" stopColor="#050608" />
        </radialGradient>
        <radialGradient id={ids.white} cx="0.35" cy="0.3" r="0.8">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.7" stopColor="#e9ebef" />
          <stop offset="1" stopColor="#b9bec8" />
        </radialGradient>
        <linearGradient id={ids.wood} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#e2c38c" />
          <stop offset="1" stopColor="#c99a5b" />
        </linearGradient>
        <linearGradient id={ids.shade} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="#0b1019" stopOpacity="0.85" />
          <stop offset="0.35" stopColor="#0b1019" stopOpacity="0" />
        </linearGradient>
      </defs>
      <rect width="320" height="180" fill="#0b1019" />
      <rect width="320" height="180" fill={`url(#${ids.bg})`} />
      <rect width="320" height="180" fill={`url(#${ids.glow})`} />
      {art}
    </svg>
  )
}
