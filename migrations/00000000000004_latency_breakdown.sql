-- Adds per-request latency breakdown so analytics can distinguish provider
-- response time from gateway overhead.

ALTER TABLE public.requests_log
    ADD COLUMN IF NOT EXISTS provider_latency_ms integer DEFAULT 0,
    ADD COLUMN IF NOT EXISTS gateway_latency_ms integer DEFAULT 0;
