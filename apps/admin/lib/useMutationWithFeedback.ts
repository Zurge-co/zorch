"use client";

import { useCallback, useState } from "react";
import { toast } from "@/components/ui/toast";
import { confirmDialog } from "@/components/ui/confirm-dialog";

/**
 * Wraps every per-page mutation handler in this app:
 *
 *   const handleFoo = async () => {
 *     if (!pred) return;
 *     setMutating(true);
 *     try {
 *       await apiCall(...);
 *       await refetch();
 *     } catch (err) {
 *       console.error(err);
 *       toast.error(err instanceof Error ? err.message : "Failed to …");
 *     } finally {
 *       setMutating(false);
 *     }
 *   };
 *
 * Single source of truth for "busy flag + try/catch + refetch + toast".
 * Errors are surfaced via the toast system; confirms go through a Dialog-based
 * `confirmDialog()` (no native browser dialogs).
 *
 * Usage:
 *   const { mutate, mutating } = useMutationWithFeedback({ refetch });
 *   const handleDelete = (x) =>
 *     mutate(() => deleteProvider(x), {
 *       confirm: "Sure?",
 *       errorPrefix: "Failed to delete provider",
 *     });
 */
export interface UseMutationWithFeedbackOptions {
  refetch?: () => Promise<void> | void;
}

export interface MutateOptions {
  confirm?: string;
  errorPrefix?: string;
  danger?: boolean;
  confirmLabel?: string;
  cancelLabel?: string;
  success?: string;
}

export function useMutationWithFeedback(options: UseMutationWithFeedbackOptions = {}) {
  const { refetch } = options;
  const [mutating, setMutating] = useState(false);

  const mutate = useCallback(
    async <T,>(action: () => Promise<T>, mutateOptions?: MutateOptions): Promise<T | undefined> => {
      if (mutating) return;

      if (mutateOptions?.confirm) {
        const ok = await confirmDialog({
          title: "Are you sure?",
          description: mutateOptions.confirm,
          danger: mutateOptions.danger ?? false,
          confirmLabel: mutateOptions.confirmLabel,
          cancelLabel: mutateOptions.cancelLabel,
        });
        if (!ok) return;
      }

      const errorPrefix = mutateOptions?.errorPrefix ?? "Action failed";
      setMutating(true);
      try {
        const result = await action();
        if (refetch) {
          await refetch();
        }
        if (mutateOptions?.success) {
          toast.success(mutateOptions.success);
        }
        return result;
      } catch (err) {
        console.error(err);
        toast.error(err instanceof Error ? err.message : errorPrefix, { duration: 6000 });
        return undefined;
      } finally {
        setMutating(false);
      }
    },
    [mutating, refetch],
  );

  return { mutate, mutating };
}
