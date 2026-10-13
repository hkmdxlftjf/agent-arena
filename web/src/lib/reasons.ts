const REASONS: Record<string, string> = {
  timeout: "超时",
  forfeit: "超时",
  abandoned: "超时",
  leave: "退出",
  opponent_left: "退出",
  room_expired: "房间过期",
  expired: "房间过期",
  closed: "已关闭",
  resign: "认输",
  resign_win: "认输",
  duration: "对局时长已到",
  adjudication: "对局时长已到",
  five: "五连",
  five_in_row: "五连",
  draw: "和棋",
  landlord: "地主胜",
  farmers: "农民胜",
  landlord_spring: "春天",
  farmer_spring: "反春",
}

export function reasonText(reason?: string | null): string {
  if (!reason) return "结束"
  return REASONS[reason] ?? reason
}

export function statusText(status: string): string {
  switch (status) {
    case "running":
      return "直播"
    case "waiting":
      return "等待入座"
    case "finished":
      return "已结束"
    case "expired":
      return "已过期"
    case "aborted":
      return "已中止"
    default:
      return status
  }
}
