CREATE TABLE IF NOT EXISTS requests_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL,
    organization_id UUID REFERENCES organizations(id),
    api_key_id UUID REFERENCES api_keys(id),
    provider TEXT,
    model TEXT,
    status_code INT,
    latency_ms INT,
    input_tokens INT,
    output_tokens INT,
    provider_cost DOUBLE PRECISION,
    markup_percent DOUBLE PRECISION,
    total_cost DOUBLE PRECISION,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX idx_requests_log_created_at ON requests_log(created_at);
