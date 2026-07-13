CREATE TABLE IF NOT EXISTS inspector_requests (
    timestamp DateTime64(3),
    request_id UUID,
    organization_id Nullable(UUID),
    api_key_id Nullable(UUID),
    provider_id String,
    model String,
    input_tokens UInt32,
    output_tokens UInt32,
    latency_ms UInt32,
    status_code UInt16,
    error_message Nullable(String),
    capture_level String,
    middleware_metadata Nullable(String)
) ENGINE = MergeTree()
ORDER BY (timestamp, request_id);
