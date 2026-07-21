import type { AgentMeterOptions, ToolCall, TrackOptions } from "./types.js";
export declare class AgentMeter {
    private apiKey;
    private endpoint;
    private ide;
    private agent?;
    private buffer;
    private timer;
    private beforeExitHandler;
    private closed;
    constructor(opts?: AgentMeterOptions);
    track(toolName: string, opts?: TrackOptions): ToolCall;
    finish(span: ToolCall, ok?: boolean, error?: string): void;
    flush(): Promise<number>;
    shutdown(): Promise<void>;
    private buildOtlpPayload;
}
