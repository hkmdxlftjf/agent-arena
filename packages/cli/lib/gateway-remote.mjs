// Streamable HTTP client. A lost response is never retried here.
export class RemoteError extends Error {
  constructor(code, message, data) {
    super(message);
    this.code = code;
    this.data = data;
  }
}

export function toolValue(result) {
  let value = result?.structuredContent;
  if (value === undefined) {
    const text = result?.content?.find(item => item.type === "text")?.text;
    try { value = JSON.parse(text); }
    catch { throw new RemoteError("remote_format", "Remote tool did not return JSON"); }
  }
  if (result.isError && !value?.error) {
    return { error: "remote_tool_error" };
  }
  return value;
}

export class RemoteClient {
  constructor({ baseUrl, token, onNotification = () => {} }) {
    this.url = `${baseUrl.replace(/\/+$/, "")}/mcp`;
    this.token = token;
    this.onNotification = onNotification;
    this.sequence = 0;
    this.session = undefined;
    this.ready = undefined;
    this.protocol = "2025-03-26";
  }

  async initialize() {
    if (!this.ready) {
      this.ready = this.request("initialize", {
        protocolVersion: this.protocol,
        capabilities: {},
        clientInfo: { name: "agent-arena-local", version: "1.0.0" },
      }).then(async result => {
        this.protocol = result.protocolVersion || this.protocol;
        await this.request("notifications/initialized", {}, true);
        return result;
      }).catch(error => {
        this.ready = undefined;
        throw error;
      });
    }
    return this.ready;
  }

  async call(method, params = {}) {
    await this.initialize();
    return this.request(method, params);
  }

  async tool(name, args) {
    return toolValue(await this.call("tools/call", { name, arguments: args }));
  }

  async request(method, params, notification = false) {
    const id = ++this.sequence;
    const headers = {
      authorization: `Bearer ${this.token}`,
      "content-type": "application/json",
      accept: "application/json, text/event-stream",
      "mcp-protocol-version": this.protocol,
    };
    if (this.session) headers["mcp-session-id"] = this.session;
    // Keep the deadline active through JSON/SSE body consumption, not just headers.
    // Remote waits can hold for 600 seconds; ordinary calls must release the journal.
    const timeoutMs = method === "tools/call" && params.name === "wait_for_turn" ? 660_000 : 30_000;
    const response = await fetch(this.url, {
      method: "POST",
      headers,
      redirect: "error",
      signal: AbortSignal.timeout(timeoutMs),
      body: JSON.stringify({ jsonrpc: "2.0", ...(notification ? {} : { id }), method, params }),
    });
    const session = response.headers.get("mcp-session-id");
    if (session) this.session = session;
    if (!response.ok) {
      if (response.status === 404 && this.session) {
        this.session = undefined;
        this.ready = undefined;
      }
      let data;
      try { data = await response.json(); } catch { /* HTTP status remains authoritative. */ }
      throw new RemoteError(-32000, `Remote HTTP ${response.status}`, data);
    }
    if (notification) {
      await response.body?.cancel();
      return {};
    }
    let message;
    if (response.headers.get("content-type")?.includes("text/event-stream")) {
      message = await this.readEvents(response, id);
    } else {
      message = await response.json();
    }
    if (message?.id !== id) throw new RemoteError(-32000, "Remote response ID mismatch");
    if (message.error) {
      throw new RemoteError(message.error.code, message.error.message, message.error.data);
    }
    if (!Object.hasOwn(message, "result")) throw new RemoteError(-32000, "Missing remote result");
    return message.result;
  }

  async readEvents(response, id) {
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "", data = [];
    const consume = line => {
      if (line.endsWith("\r")) line = line.slice(0, -1);
      if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
      if (line !== "" || !data.length) return;
      const message = JSON.parse(data.join("\n"));
      data = [];
      if (message.id === id) return message;
      if (message.method && !Object.hasOwn(message, "id")) this.onNotification(message);
    };
    try {
      for (;;) {
        const { value, done } = await reader.read();
        buffer += decoder.decode(value, { stream: !done });
        let end;
        while ((end = buffer.indexOf("\n")) !== -1) {
          const message = consume(buffer.slice(0, end));
          buffer = buffer.slice(end + 1);
          if (message) return message;
        }
        if (done) {
          if (buffer) {
            const message = consume(buffer);
            if (message) return message;
          }
          const message = consume("");
          if (message) return message;
          throw new RemoteError(-32000, "Remote event stream ended without a result");
        }
      }
    } finally {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
  }
}
