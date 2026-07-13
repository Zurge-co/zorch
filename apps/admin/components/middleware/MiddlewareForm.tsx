"use client";

import React from "react";
import dynamic from "next/dynamic";
import { useTheme } from "next-themes";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { MiddlewareRunPanel, RunPayload, RunResult } from "./MiddlewareRunPanel";

const Editor = dynamic(() => import("@monaco-editor/react"), { ssr: false });

export interface MiddlewareFormData {
  name: string;
  phase: string;
  priority: number;
  failureMode: string;
  enabled: boolean;
  source: string;
  maxOperations: number;
  maxStringSize: number;
  maxArraySize: number;
  maxMapSize: number;
  maxCallStackDepth: number;
}

const PHASE_OPTIONS = [
  { value: "request.pre_governance", label: "Pre-Governance" },
  { value: "request.pre_upstream", label: "Pre-Upstream" },
];

const DEFAULT_SOURCE = `fn run(ctx, input, config) {
    let body = input.body;

    // 1. Set a default model if none provided
    if config.defaultModel != () && body.model == () {
        body.model = config.defaultModel;
    }

    // 2. Block forbidden models
    if config.blockedModels != () && config.blockedModels.contains(body.model) {
        return #{
            action: "block",
            status_code: 400,
            message: "Model '" + body.model + "' is not allowed",
            metadata: #{ reason: "blocked_model", model: body.model }
        };
    }

    // 3. Require a system prompt
    if config.requireSystemPrompt == true {
        let has_system = false;
        if body.messages != () {
            for msg in body.messages {
                if msg.role == "system" {
                    has_system = true;
                    break;
                }
            }
        }
        if !has_system {
            return #{
                action: "block",
                status_code: 400,
                message: "System prompt is required",
                metadata: #{ reason: "missing_system_prompt" }
            };
        }
    }

    // 4. Clamp max_tokens
    if config.maxAllowedTokens != () && body.max_tokens != () && body.max_tokens > config.maxAllowedTokens {
        body.max_tokens = config.maxAllowedTokens;
    }

    return #{
        action: "continue",
        body: body,
        metadata: #{
            model_overridden: body.model,
            gateway: "zorch"
        }
    };
}`;

interface ValidationResult {
  valid: boolean;
  error?: string;
}

interface MiddlewareFormProps {
  defaultValues?: Partial<MiddlewareFormData>;
  onSubmit: (data: MiddlewareFormData) => void;
  onCancel: () => void;
  submitLabel: string;
  loading?: boolean;
  onValidate?: (source: string) => Promise<ValidationResult>;
  onRun?: (payload: RunPayload) => Promise<RunResult>;
}

export function MiddlewareForm({
  defaultValues,
  onSubmit,
  onCancel,
  submitLabel,
  loading,
  onValidate,
  onRun,
}: MiddlewareFormProps) {
  const [name, setName] = React.useState(defaultValues?.name ?? "");
  const [phase, setPhase] = React.useState(defaultValues?.phase ?? "request.pre_upstream");
  const [priority, setPriority] = React.useState(defaultValues?.priority ?? 100);
  const [failureMode, setFailureMode] = React.useState(defaultValues?.failureMode ?? "fail_closed");
  const [enabled, setEnabled] = React.useState(defaultValues?.enabled ?? true);
  const [source, setSource] = React.useState(defaultValues?.source ?? DEFAULT_SOURCE);
  const [maxOperations, setMaxOperations] = React.useState(defaultValues?.maxOperations ?? 1_000_000);
  const [maxStringSize, setMaxStringSize] = React.useState(defaultValues?.maxStringSize ?? 65536);
  const [maxArraySize, setMaxArraySize] = React.useState(defaultValues?.maxArraySize ?? 10_000);
  const [maxMapSize, setMaxMapSize] = React.useState(defaultValues?.maxMapSize ?? 10_000);
  const [maxCallStackDepth, setMaxCallStackDepth] = React.useState(
    defaultValues?.maxCallStackDepth ?? 64
  );
  const [validation, setValidation] = React.useState<ValidationResult | null>(null);
  const [validating, setValidating] = React.useState(false);
  const { theme, systemTheme } = useTheme();

  const activeTheme = theme === "system" ? systemTheme : theme;
  const editorTheme = activeTheme === "dark" ? "vs-dark" : "vs";

  const handleValidate = async () => {
    if (!onValidate) return;
    setValidating(true);
    setValidation(null);
    try {
      const result = await onValidate(source);
      setValidation(result);
    } catch (err) {
      setValidation({
        valid: false,
        error: err instanceof Error ? err.message : "Validation failed",
      });
    } finally {
      setValidating(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      name,
      phase,
      priority,
      failureMode,
      enabled,
      source,
      maxOperations,
      maxStringSize,
      maxArraySize,
      maxMapSize,
      maxCallStackDepth,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="space-y-2">
          <Label htmlFor="name">Name</Label>
          <Input
            id="name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Token Reducer"
            disabled={loading}
            required
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="phase">Phase</Label>
          <select
            id="phase"
            value={phase}
            onChange={(e) => setPhase(e.target.value)}
            disabled={loading}
            className="h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
          >
            {PHASE_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </div>

        <div className="space-y-2">
          <Label htmlFor="priority">Priority</Label>
          <Input
            id="priority"
            type="number"
            value={priority}
            onChange={(e) => setPriority(parseInt(e.target.value) || 0)}
            placeholder="100"
            disabled={loading}
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="failureMode">Failure Mode</Label>
          <select
            id="failureMode"
            value={failureMode}
            onChange={(e) => setFailureMode(e.target.value)}
            disabled={loading}
            className="h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
          >
            <option value="fail_open">Fail Open (log error, continue)</option>
            <option value="fail_closed">Fail Closed (block request)</option>
          </select>
        </div>
      </div>

      <div className="flex items-center gap-3 rounded-lg border border-border bg-muted/40 px-4 py-3 w-fit">
        <Switch id="enabled" checked={enabled} onCheckedChange={setEnabled} disabled={loading} />
        <Label htmlFor="enabled" className="text-sm font-medium">
          Enabled
        </Label>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
        <div className="lg:col-span-3 space-y-2">
          <div className="flex items-center justify-between">
            <Label htmlFor="source">Rhai Script</Label>
            <div className="flex items-center gap-2">
              {onValidate && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleValidate}
                  disabled={validating || !source.trim()}
                >
                  {validating ? "Validating..." : "Validate Script"}
                </Button>
              )}
            </div>
          </div>
          <div className="rounded-md border border-input overflow-hidden min-h-[600px]">
            <Editor
              height="600px"
              defaultLanguage="rust"
              value={source}
              onChange={(value) => {
                setSource(value ?? "");
                setValidation(null);
              }}
              theme={editorTheme}
              options={{
                minimap: { enabled: true },
                fontSize: 13,
                lineNumbers: "on",
                roundedSelection: false,
                scrollBeyondLastLine: false,
                automaticLayout: true,
                padding: { top: 16 },
                wordWrap: "on",
              }}
              loading={
                <div className="h-[600px] flex items-center justify-center text-sm text-muted-foreground">
                  Loading editor...
                </div>
              }
            />
          </div>
          <p className="text-xs text-muted-foreground">
            The script must define a function named <code>run</code> with the signature{" "}
            <code>fn run(ctx, input, config)</code>.
          </p>
          {validation && (
            <p
              className={`text-xs ${
                validation.valid ? "text-green-600" : "text-destructive"
              }`}
            >
              {validation.valid
                ? "Script is valid"
                : `Invalid script: ${validation.error}`}
            </p>
          )}

          <div className="grid grid-cols-2 md:grid-cols-5 gap-4 pt-2">
            <div className="space-y-2">
              <Label htmlFor="maxOperations">Max Operations</Label>
              <Input
                id="maxOperations"
                type="number"
                value={maxOperations}
                onChange={(e) => setMaxOperations(parseInt(e.target.value) || 0)}
                disabled={loading}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="maxStringSize">Max String Size</Label>
              <Input
                id="maxStringSize"
                type="number"
                value={maxStringSize}
                onChange={(e) => setMaxStringSize(parseInt(e.target.value) || 0)}
                disabled={loading}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="maxArraySize">Max Array Size</Label>
              <Input
                id="maxArraySize"
                type="number"
                value={maxArraySize}
                onChange={(e) => setMaxArraySize(parseInt(e.target.value) || 0)}
                disabled={loading}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="maxMapSize">Max Map Size</Label>
              <Input
                id="maxMapSize"
                type="number"
                value={maxMapSize}
                onChange={(e) => setMaxMapSize(parseInt(e.target.value) || 0)}
                disabled={loading}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="maxCallStackDepth">Max Call Stack Depth</Label>
              <Input
                id="maxCallStackDepth"
                type="number"
                value={maxCallStackDepth}
                onChange={(e) => setMaxCallStackDepth(parseInt(e.target.value) || 0)}
                disabled={loading}
              />
            </div>
          </div>
        </div>

        <div className="lg:col-span-2">
          {onRun && (
            <MiddlewareRunPanel
              source={source}
              maxOperations={maxOperations}
              maxStringSize={maxStringSize}
              maxArraySize={maxArraySize}
              maxMapSize={maxMapSize}
              maxCallStackDepth={maxCallStackDepth}
              onRun={onRun}
              loading={loading}
            />
          )}
        </div>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={loading}>
          Cancel
        </Button>
        <Button type="submit" disabled={loading || !name.trim() || !source.trim()}>
          {loading ? "Saving..." : submitLabel}
        </Button>
      </div>
    </form>
  );
}

export type { RunPayload, RunResult } from "./MiddlewareRunPanel";
