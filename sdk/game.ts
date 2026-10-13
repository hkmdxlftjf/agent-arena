// Types for uploaded rules. The sandbox executes JavaScript, not TypeScript.
// defineGame is injected by the host. Do not use Date, Math.random, eval, or network.

export type Seat = number;

export interface Viewer {
  seat?: Seat | null;
  omniscient?: boolean;
}

export interface Action<T = unknown> {
  type: string;
  data?: T;
}

export interface Visibility {
  public: boolean;
  seats?: Seat[];
}

export interface GameEvent<T = unknown> {
  type: string;
  data?: T;
  visibility?: Visibility;
}

export type TimeoutPolicy = "auto_pass" | "auto_action" | "forfeit";

export interface TimeoutDecision {
  policy: TimeoutPolicy;
  action?: Action;
}

export type Trait = "board" | "card" | "tile" | "hidden_info" | "chance" | "teams" | "multi_phase";

export interface TimeControl {
  perActionMs: number;
  maxConsecutiveTimeouts: number;
  graceMs: number;
}

export interface Manifest {
  id: string;
  name: string;
  version: string;
  description?: string;
  minPlayers: number;
  maxPlayers: number;
  traits: Trait[];
  variants: { id: string; name: string; options?: Record<string, unknown> }[];
  defaultTime: TimeControl;
  timeoutPolicy: TimeoutPolicy;
  allowedTimeoutPolicies: TimeoutPolicy[];
  actionSchema: object;
  rulesMarkdown: string;
  renderer: "grid-board" | "card-table" | "json-inspector";
  rated: boolean;
  renderSchema?: Record<string, unknown>;
}

export interface Config {
  players: number;
  variant: string;
  options?: Record<string, unknown>;
}

export interface RNG {
  uint32(): number;
  int(n: number): number;
  shuffle<T>(arr: T[]): T[];
}

export interface Result {
  scores: number[];
  ranking: Seat[][];
  teams?: Seat[][];
  reason: string;
}

export interface Game<S = unknown, A = unknown, V = unknown> {
  manifest: Manifest;
  setup(cfg: Config, rng: RNG): { state: S; events?: GameEvent[] };
  toAct(s: S): Seat[];
  view(s: S, viewer: Viewer): V;
  legalActions(s: S, seat: Seat): Action<A>[];
  apply(s: S, seat: Seat, a: Action<A>, rng: RNG): { state: S; events?: GameEvent[] };
  isTerminal(s: S): boolean;
  result(s: S): Result;
  timeoutAction(s: S, seat: Seat): TimeoutDecision;
}

export function defineGame<S, A, V>(g: Game<S, A, V>): Game<S, A, V> {
  return g;
}
