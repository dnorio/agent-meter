import { describe, it, mock, beforeEach } from "node:test";
import assert from "node:assert/strict";

// We test the built JS, so build first: npm run build
// Then: npm test

describe("AgentMeter", () => {
  let AgentMeter;

  beforeEach(async () => {
    ({ AgentMeter } = await import("../dist/index.js"));
  });

  it("track creates a span with correct fields", () => {
    const am = new AgentMeter({ apiKey: "test", endpoint: "http://localhost:3000", ide: "test-ide" });
    const span = am.track("my_tool", { model: "gpt-4o" });
    assert.equal(span.toolName, "my_tool");
    assert.equal(span.model, "gpt-4o");
    assert.equal(span.ide, "test-ide");
    assert.equal(span.ok, true);
    assert.ok(span.traceId.length > 0);
    assert.ok(span.spanId.length > 0);
    clearInterval(am["timer"]);
  });

  it("finish sets duration and timing", () => {
    const am = new AgentMeter({ apiKey: "test", endpoint: "http://localhost:3000" });
    const span = am.track("tool_a");
    am.finish(span);
    assert.ok(span.endedAtNs !== undefined);
    assert.ok(span.durationMs !== undefined);
    assert.ok(span.durationMs >= 0);
    clearInterval(am["timer"]);
  });

  it("finish with error sets ok=false", () => {
    const am = new AgentMeter({ apiKey: "test", endpoint: "http://localhost:3000" });
    const span = am.track("tool_b");
    am.finish(span, false, "timeout");
    assert.equal(span.ok, false);
    assert.equal(span.error, "timeout");
    clearInterval(am["timer"]);
  });

  it("buildOtlpPayload produces valid structure", () => {
    const am = new AgentMeter({ apiKey: "test", endpoint: "http://localhost:3000" });
    const span = am.track("code_gen", { model: "claude-sonnet-4-20250514" });
    am.finish(span);
    const payload = am["buildOtlpPayload"]([span]);
    assert.ok(payload.resourceSpans);
    assert.equal(payload.resourceSpans[0].scopeSpans[0].spans[0].name, "code_gen");
    clearInterval(am["timer"]);
  });
});
