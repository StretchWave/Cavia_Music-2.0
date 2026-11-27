-- Cavia Connect collaborative session schema and row-level security

CREATE TABLE IF NOT EXISTS public.connect_sessions (
    session_id TEXT PRIMARY KEY,
    join_code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    host_user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    host_participant_id TEXT NOT NULL,
    host_display_name TEXT NOT NULL,
    fairness_alpha DOUBLE PRECISION NOT NULL DEFAULT 1.0 CHECK (fairness_alpha >= 0.0 AND fairness_alpha <= 5.0),
    preload_target INTEGER NOT NULL DEFAULT 10 CHECK (preload_target >= 0 AND preload_target <= 20),
    queue_revision BIGINT NOT NULL DEFAULT 0,
    session_seed BIGINT NOT NULL DEFAULT 1,
    current_entry_id TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.connect_sessions ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Authenticated users can inspect active connect sessions"
    ON public.connect_sessions FOR SELECT
    USING (auth.uid() IS NOT NULL AND (is_active OR host_user_id = auth.uid()));

CREATE POLICY "Hosts can create their own connect sessions"
    ON public.connect_sessions FOR INSERT
    WITH CHECK (auth.uid() = host_user_id);

CREATE POLICY "Hosts can update their own connect sessions"
    ON public.connect_sessions FOR UPDATE
    USING (auth.uid() = host_user_id)
    WITH CHECK (auth.uid() = host_user_id);

CREATE POLICY "Hosts can delete their own connect sessions"
    ON public.connect_sessions FOR DELETE
    USING (auth.uid() = host_user_id);

CREATE TABLE IF NOT EXISTS public.connect_participants (
    session_id TEXT NOT NULL REFERENCES public.connect_sessions(session_id) ON DELETE CASCADE,
    participant_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('host', 'guest')),
    is_connected BOOLEAN NOT NULL DEFAULT true,
    penalty_credits DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, participant_id)
);

ALTER TABLE public.connect_participants ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Session members can view participants"
    ON public.connect_participants FOR SELECT
    USING (
        EXISTS (
            SELECT 1 FROM public.connect_sessions s
            WHERE s.session_id = connect_participants.session_id
              AND (s.host_user_id = auth.uid() OR s.is_active)
        )
    );

CREATE POLICY "Users can register themselves as participants"
    ON public.connect_participants FOR INSERT
    WITH CHECK (auth.uid() = user_id);

CREATE POLICY "Members or host can update participant presence"
    ON public.connect_participants FOR UPDATE
    USING (
        auth.uid() = user_id OR EXISTS (
            SELECT 1 FROM public.connect_sessions s
            WHERE s.session_id = connect_participants.session_id AND s.host_user_id = auth.uid()
        )
    );

CREATE TABLE IF NOT EXISTS public.connect_queue_entries (
    queue_entry_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES public.connect_sessions(session_id) ON DELETE CASCADE,
    track_json JSONB NOT NULL,
    track_id TEXT NOT NULL,
    provider_id TEXT,
    canonical_provider_id TEXT,
    contributor_id TEXT NOT NULL,
    contributor_user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    contributor_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending_preload', 'ready', 'playing', 'played', 'removed')),
    priority_override BOOLEAN NOT NULL DEFAULT false,
    manual_position INTEGER,
    committed_rank INTEGER,
    created_at_seq BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.connect_queue_entries ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Participants can view session queue entries"
    ON public.connect_queue_entries FOR SELECT
    USING (
        EXISTS (
            SELECT 1 FROM public.connect_participants p
            WHERE p.session_id = connect_queue_entries.session_id
              AND p.user_id = auth.uid()
        )
    );

CREATE POLICY "Participants can contribute queue entries"
    ON public.connect_queue_entries FOR INSERT
    WITH CHECK (
        auth.uid() = contributor_user_id
        AND EXISTS (
            SELECT 1 FROM public.connect_participants p
            WHERE p.session_id = connect_queue_entries.session_id
              AND p.user_id = auth.uid()
        )
    );

CREATE POLICY "Host authoritatively updates session queue entries"
    ON public.connect_queue_entries FOR UPDATE
    USING (
        EXISTS (
            SELECT 1 FROM public.connect_sessions s
            WHERE s.session_id = connect_queue_entries.session_id AND s.host_user_id = auth.uid()
        )
    );

CREATE TABLE IF NOT EXISTS public.connect_events (
    server_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL REFERENCES public.connect_sessions(session_id) ON DELETE CASCADE,
    participant_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    base_revision BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.connect_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Participants can view connect events"
    ON public.connect_events FOR SELECT
    USING (
        EXISTS (
            SELECT 1 FROM public.connect_participants p
            WHERE p.session_id = connect_events.session_id AND p.user_id = auth.uid()
        )
    );

CREATE POLICY "Participants can append own connect events"
    ON public.connect_events FOR INSERT
    WITH CHECK (auth.uid() = user_id);
