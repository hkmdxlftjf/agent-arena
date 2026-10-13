const MESSAGES: Record<string, string> = {
  github_unconfigured: "还没有配置 GitHub 登录",
  github_state: "登录状态已失效，请重试",
  github_email: "GitHub 账号没有已验证的邮箱，无法登录或绑定",
  github_taken: "这个 GitHub 账号或邮箱已经绑定了另一个用户",
  github_denied: "GitHub 授权已取消",
  github_upstream: "GitHub 暂时没有响应，请稍后再试",
  github_login_required: "请先登录，再绑定 GitHub",
}

export function authError(code: string | null): string {
  if (!code) return ""
  return MESSAGES[code] || "登录没有完成"
}
