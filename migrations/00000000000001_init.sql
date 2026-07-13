--
-- PostgreSQL database dump
--

-- Dumped from database version 18.4
-- Dumped by pg_dump version 18.4

--
-- Name: update_middleware_configs_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_middleware_configs_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

--
-- Name: update_provider_model_pricing_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_provider_model_pricing_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

--
-- Name: api_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    organization_id uuid NOT NULL,
    key_hash text NOT NULL,
    scopes text[] DEFAULT '{}'::text[],
    expires_at timestamp with time zone,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    allowed_models text[],
    max_spend_usd double precision,
    requests_per_minute integer,
    requests_per_day integer,
    name text NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    allowed_hours_start smallint,
    allowed_hours_end smallint,
    window_timezone text,
    CONSTRAINT api_keys_allowed_hours_end_check CHECK (((allowed_hours_end IS NULL) OR ((allowed_hours_end >= 0) AND (allowed_hours_end <= 23)))),
    CONSTRAINT api_keys_allowed_hours_start_check CHECK (((allowed_hours_start IS NULL) OR ((allowed_hours_start >= 0) AND (allowed_hours_start <= 23))))
);

--
-- Name: middleware_configs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.middleware_configs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    plugin_key text NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    phase text NOT NULL,
    priority integer DEFAULT 100 NOT NULL,
    failure_mode text DEFAULT 'fail_closed'::text NOT NULL,
    scope jsonb DEFAULT '{}'::jsonb NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT middleware_configs_failure_mode_check CHECK ((failure_mode = ANY (ARRAY['fail_open'::text, 'fail_closed'::text]))),
    CONSTRAINT middleware_configs_phase_check CHECK ((phase = ANY (ARRAY['request.pre_governance'::text, 'request.pre_upstream'::text, 'response.pre_client'::text, 'inspector.pre_capture'::text])))
);

--
-- Name: middleware_plugins; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.middleware_plugins (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    plugin_key text NOT NULL,
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    runtime text DEFAULT 'builtin'::text NOT NULL,
    version text DEFAULT '1.0.0'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: middleware_runs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.middleware_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id text,
    plugin_key text NOT NULL,
    phase text NOT NULL,
    status text NOT NULL,
    action text NOT NULL,
    duration_ms integer DEFAULT 0 NOT NULL,
    body_changed boolean DEFAULT false NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: model_targets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.model_targets (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_id uuid NOT NULL,
    priority integer DEFAULT 0 NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    provider_target_model_id uuid NOT NULL
);

--
-- Name: models; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    public_name text NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);

--
-- Name: organizations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.organizations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    created_at timestamp with time zone DEFAULT now()
);

--
-- Name: provider_api_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.provider_api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    label text,
    encrypted_key text NOT NULL,
    priority integer DEFAULT 0,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);

--
-- Name: provider_model_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.provider_model_config (
    id uuid DEFAULT gen_random_uuid() CONSTRAINT provider_model_pricing_id_not_null NOT NULL,
    provider text CONSTRAINT provider_model_pricing_provider_not_null NOT NULL,
    model text CONSTRAINT provider_model_pricing_model_not_null NOT NULL,
    input_cost_per_1m double precision CONSTRAINT provider_model_pricing_input_cost_per_1k_not_null NOT NULL,
    output_cost_per_1m double precision CONSTRAINT provider_model_pricing_output_cost_per_1k_not_null NOT NULL,
    markup_percent double precision DEFAULT 0.0,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    max_context_tokens bigint DEFAULT 0 NOT NULL,
    provider_id uuid NOT NULL
);

--
-- Name: provider_target_models; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.provider_target_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    target_model text NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);

--
-- Name: providers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    base_url text NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    auth_type text NOT NULL,
    auth_header_name text,
    auth_prefix text,
    CONSTRAINT chk_providers_auth_type CHECK ((auth_type = ANY (ARRAY['bearer'::text, 'anthropic'::text, 'custom'::text]))),
    CONSTRAINT chk_providers_custom_auth_requires_header CHECK (((auth_type <> 'custom'::text) OR ((auth_header_name IS NOT NULL) AND (auth_header_name <> ''::text))))
);

--
-- Name: requests_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.requests_log (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    organization_id uuid,
    api_key_id uuid,
    provider text,
    model text,
    status_code integer,
    latency_ms integer,
    input_tokens integer,
    output_tokens integer,
    provider_cost double precision,
    markup_percent double precision,
    total_cost double precision,
    created_at timestamp with time zone DEFAULT now(),
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    error_message text,
    public_model text,
    provider_api_key_id uuid
);

--
-- Name: api_keys api_keys_key_hash_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_key_hash_key UNIQUE (key_hash);

--
-- Name: api_keys api_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_pkey PRIMARY KEY (id);

--
-- Name: middleware_configs middleware_configs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.middleware_configs
    ADD CONSTRAINT middleware_configs_pkey PRIMARY KEY (id);

--
-- Name: middleware_plugins middleware_plugins_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.middleware_plugins
    ADD CONSTRAINT middleware_plugins_pkey PRIMARY KEY (id);

--
-- Name: middleware_plugins middleware_plugins_plugin_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.middleware_plugins
    ADD CONSTRAINT middleware_plugins_plugin_key_key UNIQUE (plugin_key);

--
-- Name: middleware_runs middleware_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.middleware_runs
    ADD CONSTRAINT middleware_runs_pkey PRIMARY KEY (id);

--
-- Name: model_targets model_targets_model_id_provider_target_model_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.model_targets
    ADD CONSTRAINT model_targets_model_id_provider_target_model_id_key UNIQUE (model_id, provider_target_model_id);

--
-- Name: model_targets model_targets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.model_targets
    ADD CONSTRAINT model_targets_pkey PRIMARY KEY (id);

--
-- Name: models models_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_pkey PRIMARY KEY (id);

--
-- Name: models models_public_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_public_name_key UNIQUE (public_name);

--
-- Name: organizations organizations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);

--
-- Name: provider_api_keys provider_api_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_api_keys
    ADD CONSTRAINT provider_api_keys_pkey PRIMARY KEY (id);

--
-- Name: provider_model_config provider_model_config_provider_id_model_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_model_config
    ADD CONSTRAINT provider_model_config_provider_id_model_key UNIQUE (provider_id, model);

--
-- Name: provider_model_config provider_model_pricing_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_model_config
    ADD CONSTRAINT provider_model_pricing_pkey PRIMARY KEY (id);

--
-- Name: provider_model_config provider_model_pricing_provider_model_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_model_config
    ADD CONSTRAINT provider_model_pricing_provider_model_key UNIQUE (provider, model);

--
-- Name: provider_target_models provider_target_models_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_target_models
    ADD CONSTRAINT provider_target_models_pkey PRIMARY KEY (id);

--
-- Name: provider_target_models provider_target_models_provider_id_target_model_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_target_models
    ADD CONSTRAINT provider_target_models_provider_id_target_model_key UNIQUE (provider_id, target_model);

--
-- Name: providers providers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_pkey PRIMARY KEY (id);

--
-- Name: requests_log requests_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requests_log
    ADD CONSTRAINT requests_log_pkey PRIMARY KEY (id);

--
-- Name: idx_api_keys_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_api_keys_hash ON public.api_keys USING btree (key_hash);

--
-- Name: idx_middleware_configs_enabled_phase; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_middleware_configs_enabled_phase ON public.middleware_configs USING btree (enabled, phase, priority, plugin_key, id);

--
-- Name: idx_middleware_runs_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_middleware_runs_created_at ON public.middleware_runs USING btree (created_at DESC);

--
-- Name: idx_middleware_runs_plugin_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_middleware_runs_plugin_status ON public.middleware_runs USING btree (plugin_key, status, created_at DESC);

--
-- Name: idx_model_targets_model_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_model_targets_model_id ON public.model_targets USING btree (model_id);

--
-- Name: idx_provider_api_keys_provider_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_provider_api_keys_provider_id ON public.provider_api_keys USING btree (provider_id);

--
-- Name: idx_provider_model_config_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_provider_model_config_lookup ON public.provider_model_config USING btree (provider_id, model);

--
-- Name: idx_provider_target_models_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_provider_target_models_lookup ON public.provider_target_models USING btree (provider_id, target_model);

--
-- Name: idx_provider_target_models_provider_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_provider_target_models_provider_id ON public.provider_target_models USING btree (provider_id);

--
-- Name: idx_requests_log_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_requests_log_created_at ON public.requests_log USING btree (created_at);

--
-- Name: idx_requests_log_provider_api_key_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_requests_log_provider_api_key_id ON public.requests_log USING btree (provider_api_key_id);

--
-- Name: idx_requests_log_tags_gin; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_requests_log_tags_gin ON public.requests_log USING gin (tags);

--
-- Name: middleware_configs trg_middleware_configs_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_middleware_configs_updated_at BEFORE UPDATE ON public.middleware_configs FOR EACH ROW EXECUTE FUNCTION public.update_middleware_configs_updated_at();

--
-- Name: provider_model_config trg_provider_model_pricing_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_provider_model_pricing_updated_at BEFORE UPDATE ON public.provider_model_config FOR EACH ROW EXECUTE FUNCTION public.update_provider_model_pricing_updated_at();

--
-- Name: api_keys api_keys_organization_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

--
-- Name: middleware_configs middleware_configs_plugin_key_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.middleware_configs
    ADD CONSTRAINT middleware_configs_plugin_key_fkey FOREIGN KEY (plugin_key) REFERENCES public.middleware_plugins(plugin_key);

--
-- Name: model_targets model_targets_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.model_targets
    ADD CONSTRAINT model_targets_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.models(id) ON DELETE CASCADE;

--
-- Name: model_targets model_targets_provider_target_model_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.model_targets
    ADD CONSTRAINT model_targets_provider_target_model_id_fk FOREIGN KEY (provider_target_model_id) REFERENCES public.provider_target_models(id) ON DELETE CASCADE;

--
-- Name: provider_api_keys provider_api_keys_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_api_keys
    ADD CONSTRAINT provider_api_keys_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id) ON DELETE CASCADE;

--
-- Name: provider_model_config provider_model_config_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_model_config
    ADD CONSTRAINT provider_model_config_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id) ON DELETE CASCADE;

--
-- Name: provider_target_models provider_target_models_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.provider_target_models
    ADD CONSTRAINT provider_target_models_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id) ON DELETE CASCADE;

--
-- Name: requests_log requests_log_api_key_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requests_log
    ADD CONSTRAINT requests_log_api_key_id_fkey FOREIGN KEY (api_key_id) REFERENCES public.api_keys(id);

--
-- Name: requests_log requests_log_organization_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requests_log
    ADD CONSTRAINT requests_log_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id);

--
-- PostgreSQL database dump complete
--
