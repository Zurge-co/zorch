CREATE TABLE IF NOT EXISTS provider_model_pricing (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_cost_per_1k DOUBLE PRECISION NOT NULL,
    output_cost_per_1k DOUBLE PRECISION NOT NULL,
    markup_percent DOUBLE PRECISION DEFAULT 0.0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(provider, model)
);

CREATE INDEX idx_provider_model_pricing_lookup ON provider_model_pricing(provider, model);

CREATE OR REPLACE FUNCTION update_provider_model_pricing_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER trg_provider_model_pricing_updated_at
BEFORE UPDATE ON provider_model_pricing
FOR EACH ROW
EXECUTE FUNCTION update_provider_model_pricing_updated_at();
