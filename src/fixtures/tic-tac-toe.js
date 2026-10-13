// 井字棋。上传规则示例：3×3，横竖斜三子一线即胜，下满则和。
defineGame({
  manifest: {
    id: "tic-tac-toe",
    name: "井字棋",
    version: "1.0.0",
    description: "两人轮流落子，三子一线即胜。",
    minPlayers: 2,
    maxPlayers: 2,
    traits: ["board"],
    variants: [{ id: "3", name: "3×3" }],
    defaultTime: { perActionMs: 60000, maxConsecutiveTimeouts: 3, graceMs: 2000 },
    timeoutPolicy: "auto_action",
    allowedTimeoutPolicies: ["auto_action", "forfeit"],
    actionSchema: { type: "object" },
    rulesMarkdown: "双方轮流在 3×3 的空位落子，先手为 X。横、竖或斜线上连成三子者获胜。棋盘下满且无人连成三子则和棋。",
    renderer: "grid-board",
    rated: false,
    renderSchema: { marks: ["X", "O"] }
  },
  setup() {
    return { state: { cells: [0, 0, 0, 0, 0, 0, 0, 0, 0], turn: 0, winner: null, reason: null } };
  },
  toAct(s) {
    return s.winner == null ? [s.turn] : [];
  },
  view(s) {
    const files = "abc";
    const cells = [];
    for (let i = 0; i < 9; i++) {
      if (!s.cells[i]) continue;
      const at = files[i % 3] + (3 - Math.floor(i / 3));
      cells.push({ at: at, seat: s.cells[i] - 1, label: s.cells[i] === 1 ? "X" : "O" });
    }
    return {
      renderer: "grid-board",
      size: 3,
      cells: cells,
      toAct: s.winner == null ? [s.turn] : [],
      winner: s.winner,
      reason: s.reason
    };
  },
  legalActions(s, seat) {
    const actions = [];
    if (s.winner == null && seat === s.turn) {
      const files = "abc";
      for (let i = 0; i < 9; i++) {
        if (!s.cells[i]) actions.push({ type: "place", data: { at: files[i % 3] + (3 - Math.floor(i / 3)) } });
      }
    }
    actions.push({ type: "resign" });
    return actions;
  },
  apply(s, seat, action) {
    const next = { cells: s.cells.slice(), turn: s.turn, winner: s.winner, reason: s.reason };
    if (action.type === "resign") {
      next.winner = 1 - seat;
      next.reason = "resign";
      return { state: next };
    }
    if (s.winner != null) throw new Error("对局已结束");
    if (seat !== s.turn) throw new Error("还没轮到你");
    if (action.type !== "place") throw new Error("未知动作");
    const at = action.data && action.data.at;
    const files = "abc";
    const f = files.indexOf(String(at || "").charAt(0));
    const r = Number(String(at || "").slice(1));
    if (f < 0 || r < 1 || r > 3) throw new Error("坐标不合法");
    const i = (3 - r) * 3 + f;
    if (next.cells[i]) throw new Error("这里有子了");
    next.cells[i] = seat + 1;
    const lines = [[0, 1, 2], [3, 4, 5], [6, 7, 8], [0, 3, 6], [1, 4, 7], [2, 5, 8], [0, 4, 8], [2, 4, 6]];
    if (lines.some((line) => line.every((k) => next.cells[k] === seat + 1))) {
      next.winner = seat;
      next.reason = "line";
    } else if (next.cells.every((c) => c)) {
      next.winner = -1;
      next.reason = "draw";
    } else {
      next.turn = 1 - next.turn;
    }
    return { state: next };
  },
  isTerminal(s) {
    return s.winner != null;
  },
  result(s) {
    if (s.winner === -1) return { scores: [0, 0], ranking: [[0, 1]], reason: "draw" };
    const w = s.winner;
    const scores = [0, 0];
    scores[w] = 1;
    scores[1 - w] = -1;
    return { scores: scores, ranking: [[w], [1 - w]], reason: s.reason || "line" };
  },
  timeoutAction(s) {
    const files = "abc";
    for (let i = 0; i < 9; i++) {
      if (!s.cells[i]) return { policy: "auto_action", action: { type: "place", data: { at: files[i % 3] + (3 - Math.floor(i / 3)) } } };
    }
    return { policy: "forfeit", action: { type: "resign" } };
  }
});
