-- Forward-only migration: Permanent Connect IDs, Host Connection Requests,
-- Temporary 24h Ignores, Saved Hosts, and Separated Connect Controls

CREATE TABLE IF NOT EXISTS connect_account_cache (
    account_id TEXT PRIMARY KEY NOT NULL,
    connect_id TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar_url TEXT,
    hosting INTEGER NOT NULL DEFAULT 0 CHECK (hosting IN (0, 1)),
    accept_join_requests INTEGER NOT NULL DEFAULT 0 CHECK (accept_join_requests IN (0, 1)),
    show_connect_id INTEGER NOT NULL DEFAULT 0 CHECK (show_connect_id IN (0, 1)),
    active_session_id TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS connect_saved_hosts (
    owner_account_id TEXT NOT NULL,
    host_account_id TEXT NOT NULL,
    host_connect_id TEXT NOT NULL,
    host_display_name TEXT NOT NULL,
    host_avatar_url TEXT,
    last_known_hosting INTEGER NOT NULL DEFAULT 0 CHECK (last_known_hosting IN (0, 1)),
    last_known_accepting INTEGER NOT NULL DEFAULT 0 CHECK (last_known_accepting IN (0, 1)),
    saved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (owner_account_id, host_account_id)
);

CREATE INDEX IF NOT EXISTS idx_connect_saved_hosts_owner
    ON connect_saved_hosts(owner_account_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS connect_ignored_accounts_cache (
    host_account_id TEXT NOT NULL,
    ignored_account_id TEXT NOT NULL,
    ignored_connect_id TEXT NOT NULL,
    ignored_display_name TEXT NOT NULL,
    ignored_avatar_url TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (host_account_id, ignored_account_id)
);

CREATE INDEX IF NOT EXISTS idx_connect_ignored_expires
    ON connect_ignored_accounts_cache(host_account_id, expires_at);

CREATE TABLE IF NOT EXISTS connect_requests_cache (
    request_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    host_account_id TEXT NOT NULL,
    requester_account_id TEXT NOT NULL,
    requester_connect_id TEXT NOT NULL,
    requester_display_name TEXT NOT NULL,
    requester_avatar_url TEXT,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'declined', 'ignored', 'closed')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    responded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_connect_requests_host
    ON connect_requests_cache(host_account_id, status, created_at DESC);

INSERT OR IGNORE INTO app_settings (key, value_json)
VALUES
    ('connect_accept_join_requests', 'false'),
    ('connect_show_connect_id', 'false');
