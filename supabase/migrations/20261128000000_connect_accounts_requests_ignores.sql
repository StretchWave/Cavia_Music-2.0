-- Cavia Connect permanent IDs, connection requests, 24h host ignores, saved hosts,
-- separated controls, rate limiting, and server-side authorization checks.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- 1. Permanent unique 6-char Connect IDs (alphabet excludes 0, O, 1, I)
CREATE OR REPLACE FUNCTION public.generate_unambiguous_connect_id()
RETURNS TEXT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    alphabet CONSTANT TEXT := 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    raw_bytes BYTEA := extensions.gen_random_bytes(6);
    result TEXT := '';
    idx INT;
BEGIN
    FOR i IN 0..5 LOOP
        idx := (get_byte(raw_bytes, i) % 32) + 1;
        result := result || substr(alphabet, idx, 1);
    END LOOP;
    RETURN result;
END;
$$;

CREATE TABLE IF NOT EXISTS public.connect_accounts (
    account_id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    connect_id CHAR(6) NOT NULL UNIQUE CHECK (connect_id ~ '^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{6}$'),
    display_name TEXT NOT NULL CHECK (char_length(btrim(display_name)) BETWEEN 1 AND 64),
    avatar_url TEXT,
    hosting BOOLEAN NOT NULL DEFAULT false,
    accept_join_requests BOOLEAN NOT NULL DEFAULT false,
    show_connect_id BOOLEAN NOT NULL DEFAULT false,
    active_session_id TEXT REFERENCES public.connect_sessions(session_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_connect_accounts_connect_id
    ON public.connect_accounts(connect_id);

-- Prevent changing permanent connect_id or account_id after initial assignment
CREATE OR REPLACE FUNCTION public.prevent_connect_id_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.account_id IS DISTINCT FROM OLD.account_id THEN
        RAISE EXCEPTION 'account_id cannot be reassigned';
    END IF;
    IF NEW.connect_id IS DISTINCT FROM OLD.connect_id THEN
        RAISE EXCEPTION 'Permanent Connect ID cannot be changed once assigned';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_prevent_connect_id_mutation ON public.connect_accounts;
CREATE TRIGGER trg_prevent_connect_id_mutation
    BEFORE UPDATE ON public.connect_accounts
    FOR EACH ROW
    EXECUTE FUNCTION public.prevent_connect_id_mutation();

ALTER TABLE public.connect_accounts ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can view their own connect account" ON public.connect_accounts;
CREATE POLICY "Users can view their own connect account"
    ON public.connect_accounts FOR SELECT
    USING (auth.uid() = account_id);

DROP POLICY IF EXISTS "Users can update their own non-immutable connect account fields" ON public.connect_accounts;
CREATE POLICY "Users can update their own non-immutable connect account fields"
    ON public.connect_accounts FOR UPDATE
    USING (auth.uid() = account_id)
    WITH CHECK (auth.uid() = account_id);

-- Automatically provision a permanent unambiguous Connect ID on new auth user signup
CREATE OR REPLACE FUNCTION public.provision_connect_account_for_user(
    p_user_id UUID,
    p_display_name TEXT DEFAULT NULL,
    p_avatar_url TEXT DEFAULT NULL
)
RETURNS public.connect_accounts
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    existing_row public.connect_accounts;
    candidate_id TEXT;
    clean_name TEXT;
    attempts INT := 0;
BEGIN
    SELECT * INTO existing_row
    FROM public.connect_accounts
    WHERE account_id = p_user_id;

    IF FOUND THEN
        RETURN existing_row;
    END IF;

    clean_name := NULLIF(btrim(COALESCE(p_display_name, '')), '');
    IF clean_name IS NULL THEN
        SELECT COALESCE(
            NULLIF(btrim(u.raw_user_meta_data ->> 'display_name'), ''),
            NULLIF(btrim(split_part(COALESCE(u.email, ''), '@', 1)), ''),
            'Cavia Listener'
        )
        INTO clean_name
        FROM auth.users u
        WHERE u.id = p_user_id;
    END IF;
    clean_name := left(COALESCE(clean_name, 'Cavia Listener'), 64);

    LOOP
        attempts := attempts + 1;
        IF attempts > 32 THEN
            RAISE EXCEPTION 'Could not allocate unique Connect ID';
        END IF;
        candidate_id := public.generate_unambiguous_connect_id();
        BEGIN
            INSERT INTO public.connect_accounts (
                account_id, connect_id, display_name, avatar_url
            ) VALUES (
                p_user_id, candidate_id, clean_name, NULLIF(btrim(COALESCE(p_avatar_url, '')), '')
            )
            RETURNING * INTO existing_row;
            RETURN existing_row;
        EXCEPTION WHEN unique_violation THEN
            SELECT * INTO existing_row
            FROM public.connect_accounts
            WHERE account_id = p_user_id;
            IF FOUND THEN
                RETURN existing_row;
            END IF;
        END;
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION public.handle_new_auth_user_connect_id()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    PERFORM public.provision_connect_account_for_user(NEW.id, NEW.raw_user_meta_data ->> 'display_name', NEW.raw_user_meta_data ->> 'avatar_url');
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_auth_user_provision_connect_id ON auth.users;
CREATE TRIGGER trg_auth_user_provision_connect_id
    AFTER INSERT ON auth.users
    FOR EACH ROW
    EXECUTE FUNCTION public.handle_new_auth_user_connect_id();

-- Backfill existing auth.users who registered prior to this migration
INSERT INTO public.connect_accounts (account_id, connect_id, display_name)
SELECT
    u.id,
    (
        SELECT c.id FROM (
            SELECT public.generate_unambiguous_connect_id() AS id
        ) c
    ),
    left(COALESCE(NULLIF(btrim(u.raw_user_meta_data ->> 'display_name'), ''), NULLIF(btrim(split_part(COALESCE(u.email, ''), '@', 1)), ''), 'Cavia Listener'), 64)
FROM auth.users u
WHERE NOT EXISTS (
    SELECT 1 FROM public.connect_accounts ca WHERE ca.account_id = u.id
)
ON CONFLICT DO NOTHING;

-- 2. Rate limiting table for lookups & connection requests
CREATE TABLE IF NOT EXISTS public.connect_rate_limits (
    account_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempt_count INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (account_id, action)
);

ALTER TABLE public.connect_rate_limits ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION public.enforce_connect_rate_limit(
    p_account_id UUID,
    p_action TEXT,
    p_max_attempts INT,
    p_window_seconds INT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    rec public.connect_rate_limits;
BEGIN
    SELECT * INTO rec
    FROM public.connect_rate_limits
    WHERE account_id = p_account_id AND action = p_action
    FOR UPDATE;

    IF NOT FOUND THEN
        INSERT INTO public.connect_rate_limits (account_id, action, window_started_at, attempt_count)
        VALUES (p_account_id, p_action, now(), 1);
        RETURN;
    END IF;

    IF rec.window_started_at + make_interval(secs => p_window_seconds) <= now() THEN
        UPDATE public.connect_rate_limits
        SET window_started_at = now(), attempt_count = 1
        WHERE account_id = p_account_id AND action = p_action;
        RETURN;
    END IF;

    IF rec.attempt_count >= p_max_attempts THEN
        RAISE EXCEPTION 'Too many % requests. Please wait a moment and try again.', p_action;
    END IF;

    UPDATE public.connect_rate_limits
    SET attempt_count = attempt_count + 1
    WHERE account_id = p_account_id AND action = p_action;
END;
$$;

-- 3. Temporary host-specific ignore records (24 hours, auto-expiring, never global ban)
CREATE TABLE IF NOT EXISTS public.connect_ignores (
    host_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    ignored_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (host_account_id, ignored_account_id),
    CHECK (host_account_id <> ignored_account_id)
);

CREATE INDEX IF NOT EXISTS idx_connect_ignores_lookup
    ON public.connect_ignores(ignored_account_id, host_account_id, expires_at);

ALTER TABLE public.connect_ignores ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Hosts can view their own active ignores" ON public.connect_ignores;
CREATE POLICY "Hosts can view their own active ignores"
    ON public.connect_ignores FOR SELECT
    USING (auth.uid() = host_account_id);

DROP POLICY IF EXISTS "Hosts can delete their own ignores" ON public.connect_ignores;
CREATE POLICY "Hosts can delete their own ignores"
    ON public.connect_ignores FOR DELETE
    USING (auth.uid() = host_account_id);

-- 4. Incoming Connection Requests
CREATE TABLE IF NOT EXISTS public.connect_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id TEXT NOT NULL REFERENCES public.connect_sessions(session_id) ON DELETE CASCADE,
    host_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    requester_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'declined', 'ignored', 'closed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    responded_at TIMESTAMPTZ,
    CHECK (host_account_id <> requester_account_id)
);

-- Enforce strictly at most ONE pending request per requester->host pair even under races
CREATE UNIQUE INDEX IF NOT EXISTS idx_connect_requests_unique_pending
    ON public.connect_requests(host_account_id, requester_account_id)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_connect_requests_host_inbox
    ON public.connect_requests(host_account_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_connect_requests_requester
    ON public.connect_requests(requester_account_id, created_at DESC);

ALTER TABLE public.connect_requests ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Host and requester can view their connection requests" ON public.connect_requests;
CREATE POLICY "Host and requester can view their connection requests"
    ON public.connect_requests FOR SELECT
    USING (auth.uid() = host_account_id OR auth.uid() = requester_account_id);

-- 5. Saved Hosts (linked strictly by stable host_account_id)
CREATE TABLE IF NOT EXISTS public.connect_saved_hosts (
    owner_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    host_account_id UUID NOT NULL REFERENCES public.connect_accounts(account_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (owner_account_id, host_account_id),
    CHECK (owner_account_id <> host_account_id)
);

ALTER TABLE public.connect_saved_hosts ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can manage their own saved hosts" ON public.connect_saved_hosts;
CREATE POLICY "Users can manage their own saved hosts"
    ON public.connect_saved_hosts FOR ALL
    USING (auth.uid() = owner_account_id)
    WITH CHECK (auth.uid() = owner_account_id);

-- 6. Tighten Connect participant/collaboration authorization:
-- Guests cannot self-approve into connect_participants without host approval!
DROP POLICY IF EXISTS "Users can register themselves as participants" ON public.connect_participants;
DROP POLICY IF EXISTS "Only session host can add participants directly" ON public.connect_participants;
CREATE POLICY "Only session host can add participants directly"
    ON public.connect_participants FOR INSERT
    WITH CHECK (
        EXISTS (
            SELECT 1 FROM public.connect_sessions s
            WHERE s.session_id = connect_participants.session_id
              AND s.host_user_id = auth.uid()
        )
    );

DROP POLICY IF EXISTS "Participants can contribute queue entries" ON public.connect_queue_entries;
DROP POLICY IF EXISTS "Approved non-ignored participants can contribute queue entries" ON public.connect_queue_entries;
CREATE POLICY "Approved non-ignored participants can contribute queue entries"
    ON public.connect_queue_entries FOR INSERT
    WITH CHECK (
        auth.uid() = contributor_user_id
        AND EXISTS (
            SELECT 1
            FROM public.connect_sessions s
            JOIN public.connect_participants p
              ON p.session_id = s.session_id AND p.user_id = auth.uid()
            WHERE s.session_id = connect_queue_entries.session_id
              AND s.is_active = true
              AND NOT EXISTS (
                  SELECT 1 FROM public.connect_ignores ci
                  WHERE ci.host_account_id = s.host_user_id
                    AND ci.ignored_account_id = auth.uid()
                    AND ci.expires_at > now()
              )
        )
    );

-- ============================================================================
-- Authoritative Authenticated RPCs (SECURITY DEFINER)
-- ============================================================================

CREATE OR REPLACE FUNCTION public.ensure_connect_account(
    p_display_name TEXT DEFAULT NULL,
    p_avatar_url TEXT DEFAULT NULL
)
RETURNS TABLE (
    account_id UUID,
    connect_id TEXT,
    display_name TEXT,
    avatar_url TEXT,
    hosting BOOLEAN,
    accept_join_requests BOOLEAN,
    show_connect_id BOOLEAN,
    active_session_id TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    acc public.connect_accounts;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    acc := public.provision_connect_account_for_user(uid, p_display_name, p_avatar_url);
    RETURN QUERY
    SELECT acc.account_id, acc.connect_id::TEXT, acc.display_name, acc.avatar_url,
           acc.hosting, acc.accept_join_requests, acc.show_connect_id, acc.active_session_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.update_connect_account_settings(
    p_display_name TEXT DEFAULT NULL,
    p_avatar_url TEXT DEFAULT NULL,
    p_hosting BOOLEAN DEFAULT NULL,
    p_accept_join_requests BOOLEAN DEFAULT NULL,
    p_show_connect_id BOOLEAN DEFAULT NULL,
    p_active_session_id TEXT DEFAULT NULL
)
RETURNS TABLE (
    account_id UUID,
    connect_id TEXT,
    display_name TEXT,
    avatar_url TEXT,
    hosting BOOLEAN,
    accept_join_requests BOOLEAN,
    show_connect_id BOOLEAN,
    active_session_id TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    acc public.connect_accounts;
    clean_name TEXT;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    PERFORM public.provision_connect_account_for_user(uid, NULL, NULL);

    clean_name := NULLIF(left(btrim(COALESCE(p_display_name, '')), 64), '');

    UPDATE public.connect_accounts ca
    SET
        display_name = COALESCE(clean_name, ca.display_name),
        avatar_url = CASE
            WHEN p_avatar_url IS NOT NULL THEN NULLIF(btrim(p_avatar_url), '')
            ELSE ca.avatar_url
        END,
        hosting = COALESCE(p_hosting, ca.hosting),
        accept_join_requests = COALESCE(p_accept_join_requests, ca.accept_join_requests),
        show_connect_id = COALESCE(p_show_connect_id, ca.show_connect_id),
        active_session_id = CASE
            WHEN COALESCE(p_hosting, ca.hosting) = false THEN NULL
            WHEN p_active_session_id IS NOT NULL THEN NULLIF(btrim(p_active_session_id), '')
            ELSE ca.active_session_id
        END
    WHERE ca.account_id = uid
    RETURNING * INTO acc;

    -- Closing hosting ends the active session and closes remaining pending requests
    IF COALESCE(p_hosting, acc.hosting) = false THEN
        UPDATE public.connect_sessions
        SET is_active = false, updated_at = now()
        WHERE host_user_id = uid AND is_active = true;

        UPDATE public.connect_requests
        SET status = 'closed', responded_at = now()
        WHERE host_account_id = uid AND status = 'pending';
    END IF;

    RETURN QUERY
    SELECT acc.account_id, acc.connect_id::TEXT, acc.display_name, acc.avatar_url,
           acc.hosting, acc.accept_join_requests, acc.show_connect_id, acc.active_session_id;
END;
$$;

-- Safe Host Lookup by permanent 6-char Connect ID (rate-limited, exposes zero private data,
-- returns neutral availability without revealing whether requester is specifically ignored)
CREATE OR REPLACE FUNCTION public.lookup_connect_host(p_connect_id TEXT)
RETURNS TABLE (
    host_account_id UUID,
    connect_id TEXT,
    display_name TEXT,
    avatar_url TEXT,
    hosting BOOLEAN,
    accept_join_requests BOOLEAN,
    can_request BOOLEAN,
    availability_status TEXT,
    already_approved BOOLEAN,
    pending_request BOOLEAN
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    norm_id TEXT := upper(btrim(COALESCE(p_connect_id, '')));
    target public.connect_accounts;
    is_ignored BOOLEAN := false;
    is_approved BOOLEAN := false;
    has_pending BOOLEAN := false;
    can_send BOOLEAN := false;
    avail TEXT;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;

    PERFORM public.enforce_connect_rate_limit(uid, 'lookup_host', 25, 60);
    DELETE FROM public.connect_ignores ci WHERE ci.expires_at <= now();

    IF norm_id !~ '^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{6}$' THEN
        RETURN;
    END IF;

    SELECT * INTO target
    FROM public.connect_accounts ca
    WHERE ca.connect_id = norm_id;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    SELECT EXISTS (
        SELECT 1 FROM public.connect_ignores ci
        WHERE ci.host_account_id = target.account_id
          AND ci.ignored_account_id = uid
          AND ci.expires_at > now()
    ) INTO is_ignored;

    IF target.hosting AND target.active_session_id IS NOT NULL THEN
        SELECT EXISTS (
            SELECT 1 FROM public.connect_participants cp
            WHERE cp.session_id = target.active_session_id AND cp.user_id = uid
        ) INTO is_approved;
    END IF;

    SELECT EXISTS (
        SELECT 1 FROM public.connect_requests cr
        WHERE cr.host_account_id = target.account_id
          AND cr.requester_account_id = uid
          AND cr.status = 'pending'
    ) INTO has_pending;

    -- Do NOT leak ignore state: mask ignored guests cleanly as unavailable
    IF target.account_id = uid THEN
        can_send := false;
        avail := 'self';
    ELSIF is_ignored THEN
        can_send := false;
        avail := 'unavailable';
    ELSIF NOT target.hosting OR target.active_session_id IS NULL THEN
        can_send := false;
        avail := 'offline';
    ELSIF is_approved THEN
        can_send := false;
        avail := 'connected';
    ELSIF has_pending THEN
        can_send := false;
        avail := 'pending';
    ELSIF NOT target.accept_join_requests THEN
        can_send := false;
        avail := 'requests_disabled';
    ELSE
        can_send := true;
        avail := 'accepting_requests';
    END IF;

    RETURN QUERY SELECT
        target.account_id,
        target.connect_id::TEXT,
        target.display_name,
        target.avatar_url,
        (target.hosting AND NOT is_ignored),
        (target.hosting AND target.accept_join_requests AND NOT is_ignored),
        can_send,
        avail,
        is_approved,
        has_pending;
END;
$$;

-- Send Join Request by verified target host_account_id (rate-limited, atomic single-pending guard,
-- derives requester identity strictly from auth.uid())
CREATE OR REPLACE FUNCTION public.send_connect_request(p_host_account_id UUID)
RETURNS TABLE (
    outcome TEXT,
    request_id UUID,
    session_id TEXT,
    message TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    requester public.connect_accounts;
    host_acc public.connect_accounts;
    existing_req public.connect_requests;
    is_ignored BOOLEAN := false;
    is_member BOOLEAN := false;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    IF p_host_account_id IS NULL OR p_host_account_id = uid THEN
        RETURN QUERY SELECT 'unavailable'::TEXT, NULL::UUID, NULL::TEXT, 'Cannot send a join request to your own account.'::TEXT;
        RETURN;
    END IF;

    PERFORM public.enforce_connect_rate_limit(uid, 'send_request', 15, 60);
    requester := public.provision_connect_account_for_user(uid, NULL, NULL);
    DELETE FROM public.connect_ignores ci WHERE ci.expires_at <= now();

    SELECT * INTO host_acc
    FROM public.connect_accounts ca
    WHERE ca.account_id = p_host_account_id;

    IF NOT FOUND OR NOT host_acc.hosting OR host_acc.active_session_id IS NULL THEN
        RETURN QUERY SELECT 'offline'::TEXT, NULL::UUID, NULL::TEXT, 'Host is not currently running a Cavia Connect session.'::TEXT;
        RETURN;
    END IF;

    SELECT EXISTS (
        SELECT 1 FROM public.connect_ignores ci
        WHERE ci.host_account_id = host_acc.account_id
          AND ci.ignored_account_id = uid
          AND ci.expires_at > now()
    ) INTO is_ignored;

    IF is_ignored THEN
        -- Do not confirm whether requester is ignored
        RETURN QUERY SELECT 'unavailable'::TEXT, NULL::UUID, NULL::TEXT, 'This host is unavailable to receive your request right now.'::TEXT;
        RETURN;
    END IF;

    SELECT EXISTS (
        SELECT 1 FROM public.connect_participants cp
        WHERE cp.session_id = host_acc.active_session_id AND cp.user_id = uid
    ) INTO is_member;

    IF is_member THEN
        RETURN QUERY SELECT 'connected'::TEXT, NULL::UUID, host_acc.active_session_id, 'Already connected to this session.'::TEXT;
        RETURN;
    END IF;

    IF NOT host_acc.accept_join_requests THEN
        RETURN QUERY SELECT 'requests_disabled'::TEXT, NULL::UUID, host_acc.active_session_id, 'Host has disabled new join requests.'::TEXT;
        RETURN;
    END IF;

    -- Prevent duplicate pending request even under concurrency
    INSERT INTO public.connect_requests (
        session_id, host_account_id, requester_account_id, status
    ) VALUES (
        host_acc.active_session_id, host_acc.account_id, uid, 'pending'
    )
    ON CONFLICT (host_account_id, requester_account_id) WHERE (status = 'pending')
    DO NOTHING
    RETURNING * INTO existing_req;

    IF FOUND THEN
        RETURN QUERY SELECT 'pending'::TEXT, existing_req.request_id, host_acc.active_session_id, 'Connection request sent.'::TEXT;
        RETURN;
    END IF;

    SELECT * INTO existing_req
    FROM public.connect_requests cr
    WHERE cr.host_account_id = host_acc.account_id
      AND cr.requester_account_id = uid
      AND cr.status = 'pending'
    LIMIT 1;

    RETURN QUERY SELECT 'pending'::TEXT, existing_req.request_id, host_acc.active_session_id, 'Connection request is already pending.'::TEXT;
END;
$$;

-- Host Response RPC: accept | decline | ignore_1d
CREATE OR REPLACE FUNCTION public.respond_connect_request(
    p_request_id UUID,
    p_action TEXT
)
RETURNS TABLE (
    request_id UUID,
    status TEXT,
    requester_account_id UUID,
    requester_display_name TEXT,
    session_id TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    req public.connect_requests;
    req_acc public.connect_accounts;
    next_status TEXT;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;

    SELECT * INTO req
    FROM public.connect_requests cr
    WHERE cr.request_id = p_request_id
      AND cr.host_account_id = uid
    FOR UPDATE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Connection request not found';
    END IF;
    IF req.status <> 'pending' THEN
        RAISE EXCEPTION 'Connection request has already been handled (%).', req.status;
    END IF;

    SELECT * INTO req_acc
    FROM public.connect_accounts ca
    WHERE ca.account_id = req.requester_account_id;

    IF p_action = 'accept' THEN
        next_status := 'accepted';
        INSERT INTO public.connect_participants (
            session_id, participant_id, user_id, display_name, role, is_connected, penalty_credits
        ) VALUES (
            req.session_id,
            req.requester_account_id::TEXT,
            req.requester_account_id,
            COALESCE(req_acc.display_name, 'Guest'),
            'guest',
            true,
            0.0
        )
        ON CONFLICT (session_id, participant_id) DO UPDATE SET
            display_name = excluded.display_name,
            is_connected = true,
            last_seen_at = now();
    ELSIF p_action = 'decline' THEN
        next_status := 'declined';
    ELSIF p_action = 'ignore_1d' THEN
        next_status := 'ignored';
        INSERT INTO public.connect_ignores (
            host_account_id, ignored_account_id, created_at, expires_at
        ) VALUES (
            uid, req.requester_account_id, now(), now() + interval '24 hours'
        )
        ON CONFLICT (host_account_id, ignored_account_id) DO UPDATE SET
            created_at = now(),
            expires_at = now() + interval '24 hours';
    ELSE
        RAISE EXCEPTION 'Unsupported action: %', p_action;
    END IF;

    UPDATE public.connect_requests cr
    SET status = next_status, responded_at = now()
    WHERE cr.request_id = req.request_id;

    RETURN QUERY SELECT
        req.request_id,
        next_status,
        req.requester_account_id,
        COALESCE(req_acc.display_name, 'Guest'),
        req.session_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_host_requests()
RETURNS TABLE (
    request_id UUID,
    session_id TEXT,
    requester_account_id UUID,
    requester_connect_id TEXT,
    requester_display_name TEXT,
    requester_avatar_url TEXT,
    status TEXT,
    created_at TIMESTAMPTZ,
    responded_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    RETURN QUERY
    SELECT
        cr.request_id,
        cr.session_id,
        cr.requester_account_id,
        ca.connect_id::TEXT,
        ca.display_name,
        ca.avatar_url,
        cr.status,
        cr.created_at,
        cr.responded_at
    FROM public.connect_requests cr
    JOIN public.connect_accounts ca ON ca.account_id = cr.requester_account_id
    WHERE cr.host_account_id = uid
    ORDER BY
        CASE WHEN cr.status = 'pending' THEN 0 ELSE 1 END,
        cr.created_at DESC
    LIMIT 100;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_ignored_accounts()
RETURNS TABLE (
    ignored_account_id UUID,
    connect_id TEXT,
    display_name TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    DELETE FROM public.connect_ignores ci WHERE ci.expires_at <= now();
    RETURN QUERY
    SELECT
        ci.ignored_account_id,
        ca.connect_id::TEXT,
        ca.display_name,
        ca.avatar_url,
        ci.created_at,
        ci.expires_at
    FROM public.connect_ignores ci
    JOIN public.connect_accounts ca ON ca.account_id = ci.ignored_account_id
    WHERE ci.host_account_id = uid AND ci.expires_at > now()
    ORDER BY ci.expires_at ASC;
END;
$$;

CREATE OR REPLACE FUNCTION public.unignore_connect_guest(p_ignored_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    DELETE FROM public.connect_ignores
    WHERE host_account_id = uid AND ignored_account_id = p_ignored_account_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.save_connect_host(p_host_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    IF p_host_account_id = uid THEN
        RAISE EXCEPTION 'Cannot save your own account as a host';
    END IF;
    PERFORM public.provision_connect_account_for_user(uid, NULL, NULL);
    INSERT INTO public.connect_saved_hosts (owner_account_id, host_account_id)
    VALUES (uid, p_host_account_id)
    ON CONFLICT DO NOTHING;
END;
$$;

CREATE OR REPLACE FUNCTION public.remove_saved_host(p_host_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    DELETE FROM public.connect_saved_hosts
    WHERE owner_account_id = uid AND host_account_id = p_host_account_id;
END;
$$;

-- Dynamically joins connect_accounts so host display-name/avatar changes update automatically
CREATE OR REPLACE FUNCTION public.list_saved_hosts()
RETURNS TABLE (
    host_account_id UUID,
    host_connect_id TEXT,
    host_display_name TEXT,
    host_avatar_url TEXT,
    hosting BOOLEAN,
    accept_join_requests BOOLEAN,
    availability_status TEXT,
    saved_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    DELETE FROM public.connect_ignores ci WHERE ci.expires_at <= now();
    RETURN QUERY
    SELECT
        h.account_id,
        h.connect_id::TEXT,
        h.display_name,
        h.avatar_url,
        (h.hosting AND NOT COALESCE(ign.is_ignored, false)) AS hosting,
        (h.hosting AND h.accept_join_requests AND NOT COALESCE(ign.is_ignored, false)) AS accept_join_requests,
        CASE
            WHEN COALESCE(ign.is_ignored, false) THEN 'unavailable'
            WHEN NOT h.hosting OR h.active_session_id IS NULL THEN 'offline'
            WHEN COALESCE(mem.is_member, false) THEN 'connected'
            WHEN COALESCE(req.is_pending, false) THEN 'pending'
            WHEN req.last_status = 'declined' THEN 'rejected'
            WHEN req.last_status = 'ignored' THEN 'unavailable'
            WHEN NOT h.accept_join_requests THEN 'requests_disabled'
            ELSE 'accepting_requests'
        END AS availability_status,
        sh.created_at
    FROM public.connect_saved_hosts sh
    JOIN public.connect_accounts h ON h.account_id = sh.host_account_id
    LEFT JOIN LATERAL (
        SELECT true AS is_ignored FROM public.connect_ignores ci
        WHERE ci.host_account_id = h.account_id AND ci.ignored_account_id = uid AND ci.expires_at > now()
        LIMIT 1
    ) ign ON true
    LEFT JOIN LATERAL (
        SELECT true AS is_member FROM public.connect_participants cp
        WHERE cp.session_id = h.active_session_id AND cp.user_id = uid
        LIMIT 1
    ) mem ON true
    LEFT JOIN LATERAL (
        SELECT
            bool_or(cr.status = 'pending') AS is_pending,
            (ARRAY_AGG(cr.status ORDER BY cr.created_at DESC))[1] AS last_status
        FROM public.connect_requests cr
        WHERE cr.host_account_id = h.account_id AND cr.requester_account_id = uid
    ) req ON true
    WHERE sh.owner_account_id = uid
    ORDER BY h.hosting DESC, sh.created_at DESC;
END;
$$;

-- Guest-side outbound request list (masks ignore status as 'unavailable')
CREATE OR REPLACE FUNCTION public.list_my_outbound_requests()
RETURNS TABLE (
    request_id UUID,
    session_id TEXT,
    host_account_id UUID,
    host_connect_id TEXT,
    host_display_name TEXT,
    host_avatar_url TEXT,
    status TEXT,
    created_at TIMESTAMPTZ,
    responded_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    RETURN QUERY
    SELECT
        cr.request_id,
        cr.session_id,
        cr.host_account_id,
        ca.connect_id::TEXT,
        ca.display_name,
        ca.avatar_url,
        CASE WHEN cr.status = 'ignored' THEN 'unavailable' ELSE cr.status END,
        cr.created_at,
        cr.responded_at
    FROM public.connect_requests cr
    JOIN public.connect_accounts ca ON ca.account_id = cr.host_account_id
    WHERE cr.requester_account_id = uid
    ORDER BY cr.created_at DESC
    LIMIT 50;
END;
$$;

-- Starts or syncs an authoritative host Connect session while preserving independent
-- accept_join_requests and show_connect_id controls.
CREATE OR REPLACE FUNCTION public.create_host_connect_session(
    p_session_id TEXT,
    p_join_code TEXT,
    p_name TEXT,
    p_fairness_alpha DOUBLE PRECISION DEFAULT 1.0,
    p_preload_target INTEGER DEFAULT 10,
    p_session_seed BIGINT DEFAULT 1
)
RETURNS TABLE (
    account_id UUID,
    connect_id TEXT,
    display_name TEXT,
    avatar_url TEXT,
    hosting BOOLEAN,
    accept_join_requests BOOLEAN,
    show_connect_id BOOLEAN,
    active_session_id TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
#variable_conflict use_column
DECLARE
    uid UUID := auth.uid();
    acc public.connect_accounts;
BEGIN
    IF uid IS NULL THEN
        RAISE EXCEPTION 'Authentication required';
    END IF;
    acc := public.provision_connect_account_for_user(uid, NULL, NULL);

    UPDATE public.connect_sessions
    SET is_active = false, updated_at = now()
    WHERE host_user_id = uid AND is_active = true AND session_id <> p_session_id;

    INSERT INTO public.connect_sessions (
        session_id, join_code, name, host_user_id, host_participant_id,
        host_display_name, fairness_alpha, preload_target, queue_revision,
        session_seed, is_active
    ) VALUES (
        p_session_id, p_join_code, p_name, uid, uid::TEXT,
        acc.display_name, p_fairness_alpha, p_preload_target, 0,
        p_session_seed, true
    )
    ON CONFLICT (session_id) DO UPDATE SET
        name = excluded.name,
        host_display_name = excluded.host_display_name,
        is_active = true,
        updated_at = now();

    INSERT INTO public.connect_participants (
        session_id, participant_id, user_id, display_name, role, is_connected, penalty_credits
    ) VALUES (
        p_session_id, uid::TEXT, uid, acc.display_name, 'host', true, 0.0
    )
    ON CONFLICT (session_id, participant_id) DO UPDATE SET
        display_name = excluded.display_name,
        is_connected = true,
        last_seen_at = now();

    UPDATE public.connect_accounts ca
    SET hosting = true, active_session_id = p_session_id
    WHERE ca.account_id = uid
    RETURNING * INTO acc;

    RETURN QUERY
    SELECT acc.account_id, acc.connect_id::TEXT, acc.display_name, acc.avatar_url,
           acc.hosting, acc.accept_join_requests, acc.show_connect_id, acc.active_session_id;
END;
$$;

-- Restrict RPC execution strictly to authenticated accounts
REVOKE ALL ON FUNCTION public.generate_unambiguous_connect_id() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.provision_connect_account_for_user(UUID, TEXT, TEXT) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.enforce_connect_rate_limit(UUID, TEXT, INT, INT) FROM PUBLIC, anon, authenticated;

DO $$
DECLARE
    fn_sig TEXT;
BEGIN
    FOREACH fn_sig IN ARRAY ARRAY[
        'public.ensure_connect_account(TEXT, TEXT)',
        'public.update_connect_account_settings(TEXT, TEXT, BOOLEAN, BOOLEAN, BOOLEAN, TEXT)',
        'public.lookup_connect_host(TEXT)',
        'public.send_connect_request(UUID)',
        'public.respond_connect_request(UUID, TEXT)',
        'public.list_host_requests()',
        'public.list_ignored_accounts()',
        'public.unignore_connect_guest(UUID)',
        'public.save_connect_host(UUID)',
        'public.remove_saved_host(UUID)',
        'public.list_saved_hosts()',
        'public.list_my_outbound_requests()',
        'public.create_host_connect_session(TEXT, TEXT, TEXT, DOUBLE PRECISION, INTEGER, BIGINT)'
    ]
    LOOP
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC, anon', fn_sig);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO authenticated', fn_sig);
    END LOOP;
END;
$$;
