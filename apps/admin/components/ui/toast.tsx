"use client";

import * as React from "react";
import { cn } from "@/lib/utils";

type ToastVariant = "default" | "success" | "warning" | "destructive";

export interface ToastItem {
  id: string;
  variant: ToastVariant;
  title?: string;
  description?: string;
  duration?: number;
}

interface ToastSink {
  toasts: ToastItem[];
  push: (t: Omit<ToastItem, "id"> & { id?: string }) => string;
  dismiss: (id: string) => void;
  clear: () => void;
}

const ToastContext = React.createContext<ToastSink | null>(null);

// Imperative singleton for callers outside React (setTimeout callbacks, error handlers).
// Module-level — does NOT survive hard refresh; consumers sign up for that.
const sinkRef: { current: ToastSink | null } = { current: null };

let idCounter = 0;
function genId(): string {
  idCounter += 1;
  return `t-${Date.now().toString(36)}-${idCounter}`;
}

type ToastInput =
  | string
  | (Omit<ToastItem, "id"> & { id?: string; title?: string; description?: string });

function pushToast(input: ToastInput): string {
  const payload: Omit<ToastItem, "id"> & { id?: string } =
    typeof input === "string"
      ? { description: input, variant: "default" }
      : input;
  if (!sinkRef.current) {
    if (typeof window !== "undefined") {
      // Provider missing — degrade to native alert. Logged so we notice in dev.
      // eslint-disable-next-line no-alert
      window.alert(payload.description ?? payload.title ?? "");
    }
    return payload.id ?? genId();
  }
  return sinkRef.current.push(payload);
}

export const toast = Object.assign(
  (input: ToastInput) => pushToast(input),
  {
    success: (description: string, opts?: Partial<ToastItem>) =>
      pushToast({ variant: "success", description, ...opts }),
    error: (description: string, opts?: Partial<ToastItem>) =>
      pushToast({ variant: "destructive", description, ...opts }),
    warning: (description: string, opts?: Partial<ToastItem>) =>
      pushToast({ variant: "warning", description, ...opts }),
    info: (description: string, opts?: Partial<ToastItem>) =>
      pushToast({ variant: "default", description, ...opts }),
    dismiss: (id: string) => sinkRef.current?.dismiss(id),
    clear: () => sinkRef.current?.clear(),
  },
);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = React.useState<ToastItem[]>([]);

  const dismiss = React.useCallback<ToastSink["dismiss"]>((id) => {
    setToasts((cur) => cur.filter((x) => x.id !== id));
  }, []);

  const clear = React.useCallback<ToastSink["clear"]>(() => {
    setToasts([]);
  }, []);

  const push = React.useCallback<ToastSink["push"]>((t) => {
    const id = t.id ?? genId();
    const item: ToastItem = {
      ...t,
      id,
      variant: t.variant ?? "default",
      duration: t.duration ?? 4000,
    };
    setToasts((cur) => [...cur, item]);
    if (item.duration && item.duration > 0) {
      setTimeout(() => dismiss(id), item.duration);
    }
    return id;
  }, [dismiss]);

  const sink = React.useMemo<ToastSink>(
    () => ({ toasts, push, dismiss, clear }),
    [toasts, push, dismiss, clear],
  );

  React.useEffect(() => {
    sinkRef.current = sink;
    return () => {
      sinkRef.current = null;
    };
  }, [sink]);

  return (
    <ToastContext.Provider value={sink}>
      {children}
      <Toaster />
    </ToastContext.Provider>
  );
}

export function useToast(): ToastSink {
  const sink = React.useContext(ToastContext);
  if (!sink) throw new Error("useToast must be used inside <ToastProvider>");
  return sink;
}

function Toaster() {
  const sink = useToast();
  return (
    <div
      role="region"
      aria-label="Notifications"
      className="pointer-events-none fixed top-4 right-4 z-[100] flex w-full max-w-sm flex-col gap-2"
    >
      {sink.toasts.map((t) => (
        <ToastCard key={t.id} toast={t} onDismiss={() => sink.dismiss(t.id)} />
      ))}
    </div>
  );
}

const variantStyles: Record<ToastVariant, string> = {
  default: "bg-popover text-popover-foreground border-border",
  success: "bg-success/10 text-success border-success/30",
  warning: "bg-warning/10 text-warning border-warning/30",
  destructive: "bg-destructive/10 text-destructive border-destructive/30",
};

function ToastCard({ toast: t, onDismiss }: { toast: ToastItem; onDismiss: () => void }) {
  return (
    <div
      role={t.variant === "destructive" ? "alert" : "status"}
      data-variant={t.variant}
      data-slot="toast"
      className={cn(
        "pointer-events-auto flex w-full items-start gap-3 rounded-lg border p-4 text-sm shadow-md",
        variantStyles[t.variant],
      )}
    >
      <div className="flex-1 space-y-1">
        {t.title && <p className="font-semibold leading-tight">{t.title}</p>}
        {t.description && <p className="text-sm leading-snug">{t.description}</p>}
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="rounded-md p-1 text-current opacity-70 transition-opacity hover:opacity-100 focus:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M18 6L6 18M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
