"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Play, RotateCcw } from "lucide-react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";

export interface RunResult {
  success: boolean;
  action?: string;
  body?: unknown;
  headers?: Record<string, string>;
  metadata?: unknown;
  bodyChanged: boolean;
  message?: string;
  statusCode?: number;
  error?: string;
  durationMs: number;
}

export interface RunPayload {
  source: string;
  maxOperations: number;
  maxStringSize: number;
  maxArraySize: number;
  maxMapSize: number;
  maxCallStackDepth: number;
  contextJson: string;
  inputJson: string;
  extraConfigJson: string;
}

interface MiddlewareRunPanelProps {
  source: string;
  maxOperations: number;
  maxStringSize: number;
  maxArraySize: number;
  maxMapSize: number;
  maxCallStackDepth: number;
  onRun: (payload: RunPayload) => Promise<RunResult>;
  loading?: boolean;
}

const DEFAULT_CONTEXT = JSON.stringify(
  {
    requestId: "req_2vPqN9xL8mK3wZ",
    orgId: "org_acme_corp",
    apiKeyId: "key_live_7f8a9b2c",
    providerId: "openai",
    modelId: "gpt-4o",
    route: "/v1/chat/completions",
  },
  null,
  2
);

const DEFAULT_INPUT = JSON.stringify(
  {
    body: {
      model: "gpt-4o",
      messages: [
        {
          role: "system",
          content: "You are a helpful assistant.",
        },
        {
          role: "user",
          content: "Explain quantum computing in simple terms.",
        },
      ],
      temperature: 0.7,
      max_tokens: 500,
      stream: false,
    },
    headers: {
      Authorization: "Bearer sk-live-abc123xyz",
      "Content-Type": "application/json",
      "X-Request-ID": "req_2vPqN9xL8mK3wZ",
      "User-Agent": "zorch-client/1.0",
    },
  },
  null,
  2
);

const DEFAULT_EXTRA_CONFIG = JSON.stringify(
  {
    blockedModels: ["gpt-4"],
    defaultModel: "gpt-4o-mini",
    requireSystemPrompt: true,
    maxAllowedTokens: 1000,
    logMetadata: true,
  },
  null,
  2
);

export function MiddlewareRunPanel({
  source,
  maxOperations,
  maxStringSize,
  maxArraySize,
  maxMapSize,
  maxCallStackDepth,
  onRun,
  loading,
}: MiddlewareRunPanelProps) {
  const [contextJson, setContextJson] = React.useState(DEFAULT_CONTEXT);
  const [inputJson, setInputJson] = React.useState(DEFAULT_INPUT);
  const [extraConfigJson, setExtraConfigJson] = React.useState(DEFAULT_EXTRA_CONFIG);
  const [running, setRunning] = React.useState(false);
  const [result, setResult] = React.useState<RunResult | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  const handleRun = async () => {
    setRunning(true);
    setResult(null);
    setError(null);
    try {
      const payload: RunPayload = {
        source,
        maxOperations,
        maxStringSize,
        maxArraySize,
        maxMapSize,
        maxCallStackDepth,
        contextJson,
        inputJson,
        extraConfigJson,
      };
      const res = await onRun(payload);
      setResult(res);
      if (!res.success) {
        setError(res.error || "Script execution failed");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to run script");
    } finally {
      setRunning(false);
    }
  };

  const handleReset = () => {
    setContextJson(DEFAULT_CONTEXT);
    setInputJson(DEFAULT_INPUT);
    setExtraConfigJson(DEFAULT_EXTRA_CONFIG);
    setResult(null);
    setError(null);
  };

  const jsonTextareaClass =
    "min-h-[180px] w-full rounded-md border border-input bg-background px-3 py-2 text-xs font-mono resize-y focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring";

  return (
    <div className="flex flex-col h-full min-h-[600px] border border-border rounded-lg bg-card overflow-hidden">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-muted/40">
        <h3 className="text-sm font-medium">Test Runner</h3>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={handleReset}
            disabled={loading || running}
          >
            <RotateCcw size={14} className="mr-1" />
            Reset Mocks
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={handleRun}
            disabled={loading || running || !source.trim()}
          >
            <Play size={14} className="mr-1" />
            {running ? "Running..." : "Run Script"}
          </Button>
        </div>
      </div>

      <div className="flex-1 flex flex-col min-h-0">
        <Tabs defaultValue="context" className="flex flex-col flex-1 min-h-0">
          <div className="px-4 pt-3 pb-0 border-b border-border">
            <TabsList variant="line" className="w-full">
              <TabsTrigger value="context">Context</TabsTrigger>
              <TabsTrigger value="input">Input</TabsTrigger>
              <TabsTrigger value="config">Extra Config</TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="context" className="flex-1 px-4 py-3 min-h-0 overflow-auto">
            <div className="space-y-2 h-full flex flex-col">
              <Label className="text-xs text-muted-foreground">
                Mock middleware context (JSON)
              </Label>
              <textarea
                value={contextJson}
                onChange={(e) => setContextJson(e.target.value)}
                className={jsonTextareaClass + " flex-1 min-h-[180px]"}
                spellCheck={false}
              />
            </div>
          </TabsContent>

          <TabsContent value="input" className="flex-1 px-4 py-3 min-h-0 overflow-auto">
            <div className="space-y-2 h-full flex flex-col">
              <Label className="text-xs text-muted-foreground">
                Mock input body & headers (JSON)
              </Label>
              <textarea
                value={inputJson}
                onChange={(e) => setInputJson(e.target.value)}
                className={jsonTextareaClass + " flex-1 min-h-[180px]"}
                spellCheck={false}
              />
            </div>
          </TabsContent>

          <TabsContent value="config" className="flex-1 px-4 py-3 min-h-0 overflow-auto">
            <div className="space-y-2 h-full flex flex-col">
              <Label className="text-xs text-muted-foreground">
                Extra config available as <code>config</code> in script (JSON)
              </Label>
              <textarea
                value={extraConfigJson}
                onChange={(e) => setExtraConfigJson(e.target.value)}
                className={jsonTextareaClass + " flex-1 min-h-[180px]"}
                spellCheck={false}
              />
            </div>
          </TabsContent>
        </Tabs>

        <div className="border-t border-border p-4 bg-muted/20 overflow-auto">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
            Result
          </h4>

          {!result && !error && (
            <p className="text-sm text-muted-foreground">
              Click <strong>Run Script</strong> to see the output.
            </p>
          )}

          {error && !result && (
            <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          {result && (
            <div className="space-y-3">
              <div className="flex items-center gap-3">
                <span
                  className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${
                    result.success
                      ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
                      : "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400"
                  }`}
                >
                  {result.success ? "Success" : "Failed"}
                </span>
                {result.action && (
                  <span className="text-xs font-mono text-muted-foreground">
                    action: <span className="text-foreground">{result.action}</span>
                  </span>
                )}
                <span className="text-xs text-muted-foreground ml-auto">
                  {result.durationMs} ms
                </span>
              </div>

              {result.success && (
                <>
                  {result.statusCode !== undefined && result.statusCode !== null && (
                    <div className="text-xs">
                      <span className="text-muted-foreground">status_code:</span>{" "}
                      <span className="font-mono text-foreground">{result.statusCode}</span>
                    </div>
                  )}
                  {result.message && (
                    <div className="text-xs">
                      <span className="text-muted-foreground">message:</span>{" "}
                      <span className="font-mono text-foreground">{result.message}</span>
                    </div>
                  )}
                  <div className="text-xs">
                    <span className="text-muted-foreground">body_changed:</span>{" "}
                    <span className="font-mono text-foreground">
                      {result.bodyChanged ? "true" : "false"}
                    </span>
                  </div>

                  {result.body !== undefined && result.body !== null && (
                    <div className="space-y-1">
                      <span className="text-xs text-muted-foreground">body:</span>
                      <pre className="rounded-md border border-border bg-background p-2 text-xs font-mono overflow-auto max-h-[160px]">
                        {JSON.stringify(result.body, null, 2)}
                      </pre>
                    </div>
                  )}

                  {result.headers && Object.keys(result.headers).length > 0 && (
                    <div className="space-y-1">
                      <span className="text-xs text-muted-foreground">headers:</span>
                      <pre className="rounded-md border border-border bg-background p-2 text-xs font-mono overflow-auto max-h-[120px]">
                        {JSON.stringify(result.headers, null, 2)}
                      </pre>
                    </div>
                  )}

                  {result.metadata !== undefined && result.metadata !== null && (
                    <div className="space-y-1">
                      <span className="text-xs text-muted-foreground">metadata:</span>
                      <pre className="rounded-md border border-border bg-background p-2 text-xs font-mono overflow-auto max-h-[120px]">
                        {JSON.stringify(result.metadata, null, 2)}
                      </pre>
                    </div>
                  )}
                </>
              )}

              {!result.success && result.error && (
                <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                  {result.error}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
