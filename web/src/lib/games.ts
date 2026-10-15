export const ROLES: Record<string, string[]> = {
  gomoku: ["黑", "白"],
  go: ["黑", "白"],
  xiangqi: ["红", "黑"],
  junqi: ["南", "北"],
  doudizhu: ["甲", "乙", "丙"],
  "mahjong-sichuan": ["东", "南", "西", "北"],
}

const UPLOADED = ["甲", "乙", "丙", "丁"]

export function roleOf(gameId: string, seat: number): string {
  return ROLES[gameId]?.[seat] ?? UPLOADED[seat] ?? `座${seat}`
}

export const TRAIT: Record<string, string> = {
  board: "棋盘",
  card: "卡牌",
  tile: "牌类",
  hidden_info: "隐藏信息",
  chance: "随机",
  teams: "组队",
  multi_phase: "多阶段",
}

export function players(game: { minPlayers: number; maxPlayers: number }) {
  return game.minPlayers === game.maxPlayers ? `${game.minPlayers} 人` : `${game.minPlayers}–${game.maxPlayers} 人`
}

export const GAME_TONES: Record<string, string> = {
  gomoku: "#3fd9bc",
  xiangqi: "#ff7a6b",
  go: "#f3c661",
  junqi: "#7cc7ff",
  doudizhu: "#ffa24c",
  "mahjong-sichuan": "#6ee7a0",
}

export function gameTone(id: string) {
  return GAME_TONES[id] ?? "#a596ff"
}

export const GAME_NAMES: Record<string, string> = {
  overall: "总榜",
  gomoku: "五子棋",
  xiangqi: "中国象棋",
  go: "围棋",
  junqi: "军棋",
  doudizhu: "斗地主",
  "mahjong-sichuan": "四川麻将",
}
