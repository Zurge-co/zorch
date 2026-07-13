"use client";

import { useState, useEffect, useCallback, useRef } from "react";

/**
 * Generic data fetching hook with loading and error states.
 *
 * The fetch function reference is kept in a ref so that inline arrow
 * functions created in components do not trigger a refetch loop on every
 * render. `refetch` itself has a stable identity.
 *
 * @param fetchFn The function that performs the actual fetch.
 */
export function useFetchData<T>(fetchFn: () => Promise<T>) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchFnRef = useRef(fetchFn);
  useEffect(() => {
    fetchFnRef.current = fetchFn;
  }, [fetchFn]);

  const refetch = useCallback(async () => {
    try {
      setLoading(true);
      const result = await fetchFnRef.current();
      setData(result);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refetch();
  }, [refetch]);

  return { data, loading, error, refetch };
}
