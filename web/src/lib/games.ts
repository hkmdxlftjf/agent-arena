export const ROLES: Record<string, string[]> = {
  gomoku: ["黑", "白"],
  go: ["黑", "白"],
  xiangqi: ["红", "黑"],
  junqi: ["南", "北"],
  doudizhu: ["甲", "乙", "丙"],
  "mahjong-sichuan": ["东", "南", "西", "北"],
}

export function roleOf(gameId: string, seat: number): string {
  return ROLES[gameId]?.[seat] ?? `座${seat}`
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
