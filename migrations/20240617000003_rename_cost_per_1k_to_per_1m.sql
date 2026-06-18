-- Rename cost columns from per-1k to per-1m and scale existing values by 1000.
-- This changes the unit from "$/1K tokens" to "$/1M tokens" so that all
-- pricing is expressed per million tokens throughout the system.

ALTER TABLE provider_model_config
    RENAME COLUMN input_cost_per_1k TO input_cost_per_1m;

ALTER TABLE provider_model_config
    RENAME COLUMN output_cost_per_1k TO output_cost_per_1m;

-- Multiply existing per-1k values by 1000 to convert to per-1m.
UPDATE provider_model_config
    SET input_cost_per_1m = input_cost_per_1m * 1000,
        output_cost_per_1m = output_cost_per_1m * 1000;
