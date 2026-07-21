import { randomUUID } from "node:crypto";
import type { AgentMeterOptions, ToolCall, TrackOptions } from "./types.js";

const DEFAULT_ENDPOINT = "http://localhost:8081";
const FLUSH_INTERVAL_MS = 5000;
const MAX_BATCH = 100;

export class AgentMeter {
  private apiKey: string;
  private endpoint: string;
  private ide: string;
  private agent?: string;
  private buffer: ToolCall[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private beforeExitHandler: () => void;
  private closed = false;

  constructor(opts: AgentMeterOptions = {}) {
    this.apiKey = opts.apiKey ?? process.env.AGENT_METER_API_KEY ?? "";
    this.endpoint = (
      opts.endpoint ?? process.env.AGENT_METER_ENDPOINT ?? DEFAULT_ENDPOINT
    ).replace(/\/$/, "");
    this.ide = opts.ide ?? process.env.AGENT_METER_IDE ?? "node-sdk";
    this.agent = opts.agent ?? process.env.AGENT_METER_AGENT;

    const interval = opts.flushIntervalMs ?? FLUSH_INTERVAL_MS;
    this.timer = setInterval(() => this.flush(), interval);
    this.timer.unref(); // don't keep process alive

    // Flush on exit
    this.beforeExitHandler = () => {
      void this.shutdown();
    };
    process.once("beforeExit", this.beforeExitHandler);
  }

  track(toolName: string, opts: TrackOptions = {}): ToolCall {
    const span: ToolCall = {
      toolName,
      startedAtNs: process.hrtime.bigint(),
      ok: true,
      traceId: randomUUID().replace(/-/g, ""),
      spanId: randomUUID().replace(/-/g, "").slice(0, 16),
      model: opts.model,
      mcpServer: opts.mcpServer,
      ide: this.ide,
      agent: this.agent,
      conversationId: opts.conversationId,
      taskId: opts.taskId,
      parentSpanId: opts.parentSpanId,
    };
    this.buffer.push(span);
    return span;
  }

  finish(span: ToolCall, ok = true, error?: string): void {
    span.endedAtNs = process.hrtime.bigint();
    span.durationMs = Number((span.endedAtNs! - span.startedAtNs) / 1_000_000n);
    span.ok = ok;
    if (error) span.error = error;
  }

  async flush(): Promise<number> {
    if (this.buffer.length === 0) return 0;
    const batch = this.buffer.splice(0, MAX_BATCH);
    const payload = this.buildOtlpPayload(batch);

    try {
      const resp = await fetch(`${this.endpoint}/v1/traces`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(this.apiKey ? { Authorization: `Bearer ${this.apiKey}` } : {}),
        },
        body: JSON.stringify(payload),
        signal: AbortSignal.timeout(10_000),
      });
      if (!resp.ok) {
        // Re-queue on failure
        this.buffer.unshift(...batch);
        return 0;
      }
      return batch.length;
    } catch {
      this.buffer.unshift(...batch);
      return 0;
    }
  }

  async shutdown(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    process.removeListener("beforeExit", this.beforeExitHandler);
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    while (this.buffer.length > 0) {
      const sent = await this.flush();
      if (sent === 0) break;
    }
  }

  private buildOtlpPayload(spans: ToolCall[]) {
    const otlpSpans = spans.map((s) => {
      const attrs: Array<{ key: string; value: Record<string, unknown> }> = [
        { key: "tool.name", value: { stringValue: s.toolName } },
      ];
      if (s.model) attrs.push({ key: "gen_ai.request.model", value: { stringValue: s.model } });
      if (s.mcpServer) attrs.push({ key: "mcp.server", value: { stringValue: s.mcpServer } });
      if (s.ide) attrs.push({ key: "ide", value: { stringValue: s.ide } });
      if (s.agent) attrs.push({ key: "agent", value: { stringValue: s.agent } });
      if (s.conversationId) attrs.push({ key: "session.id", value: { stringValue: s.conversationId } });
      if (s.taskId) attrs.push({ key: "task.id", value: { stringValue: s.taskId } });
      if (s.estimatedInputTokens != null)
        attrs.push({ key: "gen_ai.usage.input_tokens", value: { intValue: String(s.estimatedInputTokens) } });
      if (s.estimatedOutputTokens != null)
        attrs.push({ key: "gen_ai.usage.output_tokens", value: { intValue: String(s.estimatedOutputTokens) } });
      if (s.usdCost != null) attrs.push({ key: "cost.usd", value: { doubleValue: s.usdCost } });
      if (s.error) attrs.push({ key: "error.message", value: { stringValue: s.error } });

      const startNs = s.startedAtNs.toString();
      const endNs = (s.endedAtNs ?? s.startedAtNs).toString();

      return {
        traceId: s.traceId,
        spanId: s.spanId,
        parentSpanId: s.parentSpanId ?? "",
        name: `execute_tool ${s.toolName}`,
        kind: 3,
        startTimeUnixNano: startNs,
        endTimeUnixNano: endNs,
        attributes: attrs,
        status: { code: s.ok ? 1 : 2, message: s.error ?? "" },
      };
    });

    return {
      resourceSpans: [
        {
          resource: {
            attributes: [{ key: "service.name", value: { stringValue: "agent-meter-sdk-node" } }],
          },
          scopeSpans: [
            {
              scope: { name: "agent-meter-node", version: "0.1.0" },
              spans: otlpSpans,
            },
          ],
        },
      ],
    };
  }
}
