export class ApiError extends Error {
  code: string
  constructor(code: string, message: string) {
    super(message)
    this.code = code
  }
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers)
  if (init?.body && !headers.has("content-type")) headers.set("content-type", "application/json")
  const res = await fetch(path, { credentials: "include", ...init, headers })
  const text = await res.text()
  const data = text ? (JSON.parse(text) as { error?: string; message?: string }) : null
  if (!res.ok) {
    throw new ApiError(data?.error || "http", data?.message || `请求失败（${res.status}）`)
  }
  return data as T
}
