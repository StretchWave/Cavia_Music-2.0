-- Forward-only migration: persist volume and mute state in playback_state
ALTER TABLE playback_state ADD COLUMN volume INTEGER NOT NULL DEFAULT 85;
ALTER TABLE playback_state ADD COLUMN is_muted INTEGER NOT NULL DEFAULT 0;

