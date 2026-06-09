export interface AgentMeterOptions {
  apiKey?: string;
  endpoint?: string;
  ide?: string;
  agent?: string;
  flushIntervalMs?: number;
}

export interface ToolCall {
  toolName: string;
  startedAtNs: bigint;
  endedAtNs?: bigint;
  durationMs?: number;
  ok: boolean;
  error?: string;
  model?: string;
  mcpServer?: string;
  ide?: string;
  agent?: string;
  conversationId?: string;
  taskId?: string;
  traceId: string;
  spanId: string;
  parentSpanId?: string;
  estimatedInputTokens?: number;
  estimatedOutputTokens?: number;
  usdCost?: number;
}

export interface TrackOptions {
  model?: string;
  mcpServer?: string;
  conversationId?: string;
  taskId?: string;
  parentSpanId?: string;
}
