-- Forward-only migration: Queue Preload settings and Cavia Connect collaborative state
INSERT OR IGNORE INTO app_settings (key, value_json)
VALUES
    ('preload_target', '10'),
    ('fairness_alpha', '1.0');

CREATE TABLE IF NOT EXISTS connect_sessions (
    session_id TEXT PRIMARY KEY NOT NULL,
    join_code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    host_participant_id TEXT NOT NULL,
    host_display_name TEXT NOT NULL,
    local_participant_id TEXT NOT NULL,
    local_role TEXT NOT NULL CHECK (local_role IN ('host', 'guest')),
    fairness_alpha REAL NOT NULL DEFAULT 1.0 CHECK (fairness_alpha >= 0.0 AND fairness_alpha <= 5.0),
    preload_target INTEGER NOT NULL DEFAULT 10 CHECK (preload_target >= 0 AND preload_target <= 20),
    queue_revision INTEGER NOT NULL DEFAULT 0,
    session_seed INTEGER NOT NULL DEFAULT 1,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS connect_participants (
    session_id TEXT NOT NULL REFERENCES connect_sessions(session_id) ON DELETE CASCADE,
    participant_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('host', 'guest')),
    is_connected INTEGER NOT NULL DEFAULT 1 CHECK (is_connected IN (0, 1)),
    penalty_credits REAL NOT NULL DEFAULT 0.0 CHECK (penalty_credits >= 0.0),
    joined_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (session_id, participant_id)
);

CREATE TABLE IF NOT EXISTS connect_queue_entries (
    queue_entry_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES connect_sessions(session_id) ON DELETE CASCADE,
    track_json TEXT NOT NULL,
    track_id TEXT NOT NULL,
    provider_id TEXT,
    canonical_provider_id TEXT,
    contributor_id TEXT NOT NULL,
    contributor_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending_preload', 'ready', 'playing', 'played', 'removed')),
    priority_override INTEGER NOT NULL DEFAULT 0 CHECK (priority_override IN (0, 1)),
    manual_position INTEGER,
    committed_rank INTEGER,
    created_at_seq INTEGER NOT NULL,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_connect_entries_session_status
    ON connect_queue_entries(session_id, status, created_at_seq);

CREATE INDEX IF NOT EXISTS idx_connect_entries_canonical
    ON connect_queue_entries(session_id, contributor_id, canonical_provider_id);

CREATE TABLE IF NOT EXISTS connect_contribution_events (
    event_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES connect_sessions(session_id) ON DELETE CASCADE,
    participant_id TEXT NOT NULL,
    canonical_provider_id TEXT,
    action TEXT NOT NULL CHECK (action IN ('add', 'remove', 'reorder', 'priority_override', 'played', 'sync_state')),
    payload_json TEXT NOT NULL DEFAULT '{}',
    base_revision INTEGER NOT NULL DEFAULT 0,
    synced INTEGER NOT NULL DEFAULT 0 CHECK (synced IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_connect_events_session_synced
    ON connect_contribution_events(session_id, synced, created_at);
