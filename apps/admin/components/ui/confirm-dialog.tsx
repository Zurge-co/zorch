"use client";

import * as React from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface ConfirmRequest {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface ConfirmSink {
  confirm: (req: ConfirmRequest) => Promise<boolean>;
}

const ConfirmContext = React.createContext<ConfirmSink | null>(null);
const sinkRef: { current: ConfirmSink | null } = { current: null };

export const confirmDialog = Object.assign(
  (req: ConfirmRequest) => {
    if (!sinkRef.current) {
      // Provider missing — degrade gracefully to native confirm.
      // eslint-disable-next-line no-alert
      return Promise.resolve(
        window.confirm(
          `${req.title}${req.description ? "\n\n" + req.description : ""}`,
        ),
      );
    }
    return sinkRef.current.confirm(req);
  },
  {
    danger: (
      description: string,
      opts?: { title?: string; confirmLabel?: string; cancelLabel?: string },
    ) =>
      confirmDialog({
        title: opts?.title ?? "Are you sure?",
        description,
        danger: true,
        confirmLabel: opts?.confirmLabel,
        cancelLabel: opts?.cancelLabel,
      }),
  },
);

export function ConfirmDialogProvider({ children }: { children: React.ReactNode }) {
  const [request, setRequest] = React.useState<ConfirmRequest | null>(null);
  const resolverRef = React.useRef<((v: boolean) => void) | null>(null);

  const confirm = React.useCallback<ConfirmSink["confirm"]>(
    (req) =>
      new Promise<boolean>((resolve) => {
        resolverRef.current = resolve;
        setRequest(req);
      }),
    [],
  );

  const close = React.useCallback((value: boolean) => {
    resolverRef.current?.(value);
    resolverRef.current = null;
    setRequest(null);
  }, []);

  const sink = React.useMemo<ConfirmSink>(() => ({ confirm }), [confirm]);

  React.useEffect(() => {
    sinkRef.current = sink;
    return () => {
      sinkRef.current = null;
    };
  }, [sink]);

  return (
    <ConfirmContext.Provider value={sink}>
      {children}
      <Dialog open={!!request} onOpenChange={(open) => { if (!open) close(false); }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{request?.title ?? "Are you sure?"}</DialogTitle>
            {request?.description && (
              <DialogDescription>{request.description}</DialogDescription>
            )}
          </DialogHeader>
          <DialogFooter className="gap-2 sm:justify-end">
            <Button
              variant="outline"
              onClick={() => close(false)}
              disabled={!request}
            >
              {request?.cancelLabel ?? "Cancel"}
            </Button>
            <Button
              variant={request?.danger ? "destructive" : "default"}
              onClick={() => close(true)}
              disabled={!request}
            >
              {request?.confirmLabel ?? "Confirm"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </ConfirmContext.Provider>
  );
}

export function useConfirmDialog(): ConfirmSink {
  const sink = React.useContext(ConfirmContext);
  if (!sink) throw new Error("useConfirmDialog must be used inside <ConfirmDialogProvider>");
  return sink;
}
