use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::domain::Track;

/// Default exponent alpha for `weight = 1 / (1 + pending)^alpha`.
pub const DEFAULT_FAIRNESS_ALPHA: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueEntryStatus {
    PendingPreload,
    Ready,
    Playing,
    Played,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FairnessConfig {
    pub alpha: f64,
    pub preferred_spacing: usize,
}

impl Default for FairnessConfig {
    fn default() -> Self {
        Self {
            alpha: DEFAULT_FAIRNESS_ALPHA,
            preferred_spacing: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantState {
    pub participant_id: String,
    pub display_name: String,
    pub role: ParticipantRole,
    pub is_connected: bool,
    pub penalty_credits: f64,
    pub total_added: u64,
    pub total_played: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedQueueEntry {
    pub queue_entry_id: String,
    pub track: Track,
    pub canonical_provider_id: String,
    pub contributor_id: String,
    pub contributor_name: String,
    pub role: ParticipantRole,
    pub status: QueueEntryStatus,
    pub priority_override: bool,
    pub manual_position: Option<usize>,
    pub committed: bool,
    pub committed_rank: Option<u64>,
    pub created_at_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantWeightSummary {
    pub participant_id: String,
    pub display_name: String,
    pub role: ParticipantRole,
    pub is_connected: bool,
    pub pending_songs: usize,
    pub penalty_credits: f64,
    pub weight: f64,
    pub total_added: u64,
    pub total_played: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeTierQueueView {
    /// Currently playing entry (if any). Never replaced by background fair scheduling.
    pub current_entry: Option<SharedQueueEntry>,
    /// Tier 1: All active/waiting shared pool entries (`Ready` + `PendingPreload`).
    pub shared_pool: Vec<SharedQueueEntry>,
    /// Tier 2: Calculated stable upcoming playback sequence (excluding current & unverified `PendingPreload`).
    pub upcoming_sequence: Vec<SharedQueueEntry>,
    /// Tier 3: Next `N` entries from `upcoming_sequence` targeted for cache preloading.
    pub preload_window: Vec<SharedQueueEntry>,
    /// Verified/unverified late or buffered entries waiting in `PendingPreload` ordered by fairness weight.
    pub pending_preload: Vec<SharedQueueEntry>,
    pub weights: Vec<ParticipantWeightSummary>,
}

/// Deterministic SplitMix64 PRNG so queue ordering stays reproducible across reconnects & sync.
#[derive(Debug, Clone)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_unit_f64(&mut self) -> f64 {
        let upper = self.next_u64() >> 11;
        (upper as f64) / ((1u64 << 53) as f64)
    }
}

/// Computes `weight = 1 / (1 + pending + penalty)^alpha`.
pub fn participant_weight(pending_songs: usize, penalty_credits: f64, alpha: f64) -> f64 {
    let effective = (pending_songs as f64) + penalty_credits.max(0.0);
    let clamped_alpha = alpha.clamp(0.0, 5.0);
    1.0 / (1.0 + effective).powf(clamped_alpha)
}

/// Produces a canonical provider identifier for deduplication while preserving case-sensitive
/// YouTube video IDs.
pub fn canonical_provider_id(track: &Track) -> String {
    let trimmed_id = track.id.trim();
    if track.source_kind == "provider"
        || track.provider_id.as_deref() == Some("youtube_music")
        || trimmed_id.starts_with("ytmusic:")
    {
        let raw_vid = trimmed_id.strip_prefix("ytmusic:").unwrap_or(trimmed_id);
        let clean_vid = raw_vid
            .split(&['?', '&', '#'][..])
            .next()
            .unwrap_or(raw_vid)
            .trim();
        if !clean_vid.is_empty() {
            return format!("ytmusic:{clean_vid}");
        }
    }
    format!("track:{trimmed_id}")
}

pub fn attach_entry_metadata(track: &Track, entry: &SharedQueueEntry) -> Track {
    let mut out = track.clone();
    out.queue_entry_id = Some(entry.queue_entry_id.clone());
    out.contributor_id = Some(entry.contributor_id.clone());
    out.contributor_name = Some(entry.contributor_name.clone());
    out.priority_override = if entry.priority_override {
        Some(true)
    } else {
        None
    };
    out
}

#[derive(Debug, Clone)]
pub struct FairQueueEngine {
    pub config: FairnessConfig,
    pub session_seed: u64,
    pub queue_revision: u64,
    next_seq: u64,
    next_committed_rank: u64,
    participants: HashMap<String, ParticipantState>,
    entries: Vec<SharedQueueEntry>,
    recent_played_contributors: Vec<String>,
}

impl Default for FairQueueEngine {
    fn default() -> Self {
        Self::new(FairnessConfig::default(), 1)
    }
}

impl FairQueueEngine {
    pub fn new(config: FairnessConfig, session_seed: u64) -> Self {
        Self {
            config,
            session_seed: if session_seed == 0 { 1 } else { session_seed },
            queue_revision: 0,
            next_seq: 1,
            next_committed_rank: 1,
            participants: HashMap::new(),
            entries: Vec::new(),
            recent_played_contributors: Vec::new(),
        }
    }

    pub fn set_alpha(&mut self, alpha: f64) {
        self.config.alpha = alpha.clamp(0.0, 5.0);
        self.queue_revision = self.queue_revision.saturating_add(1);
    }

    pub fn upsert_participant(
        &mut self,
        participant_id: &str,
        display_name: &str,
        role: ParticipantRole,
        is_connected: bool,
    ) {
        let entry = self
            .participants
            .entry(participant_id.to_string())
            .or_insert_with(|| ParticipantState {
                participant_id: participant_id.to_string(),
                display_name: display_name.to_string(),
                role,
                is_connected,
                penalty_credits: 0.0,
                total_added: 0,
                total_played: 0,
            });
        if !display_name.trim().is_empty() {
            entry.display_name = display_name.to_string();
        }
        entry.role = role;
        entry.is_connected = is_connected;
    }

    /// Marks participant connected/disconnected without resetting pending counts or penalty credits.
    pub fn set_participant_connected(&mut self, participant_id: &str, connected: bool) {
        if let Some(p) = self.participants.get_mut(participant_id) {
            p.is_connected = connected;
        }
    }

    pub fn participant(&self, participant_id: &str) -> Option<&ParticipantState> {
        self.participants.get(participant_id)
    }

    pub fn restore_state(
        &mut self,
        participants: Vec<ParticipantState>,
        entries: Vec<SharedQueueEntry>,
        queue_revision: u64,
    ) {
        self.participants = participants
            .into_iter()
            .map(|p| (p.participant_id.clone(), p))
            .collect();
        self.next_seq = entries
            .iter()
            .map(|e| e.created_at_seq)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.next_committed_rank = entries
            .iter()
            .filter_map(|e| e.committed_rank)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.entries = entries;
        self.queue_revision = queue_revision;
    }

    pub fn all_entries(&self) -> &[SharedQueueEntry] {
        &self.entries
    }

    /// Pending song count for a participant (counts active Ready + PendingPreload entries).
    pub fn pending_count_for(&self, participant_id: &str) -> usize {
        self.entries
            .iter()
            .filter(|e| {
                e.contributor_id == participant_id
                    && matches!(
                        e.status,
                        QueueEntryStatus::PendingPreload | QueueEntryStatus::Ready
                    )
            })
            .count()
    }

    pub fn validate_contribution(
        &self,
        role: ParticipantRole,
        track: &Track,
    ) -> Result<String, String> {
        if role == ParticipantRole::Guest {
            let is_yt = track.source_kind == "provider"
                || track.provider_id.as_deref() == Some("youtube_music")
                || track.id.starts_with("ytmusic:");
            if !is_yt {
                return Err(
                    "Guests can only contribute YouTube Music tracks in Cavia Connect sessions."
                        .to_string(),
                );
            }
        }

        let canon = canonical_provider_id(track);
        if role == ParticipantRole::Guest {
            let duplicate = self.entries.iter().any(|entry| {
                matches!(
                    entry.status,
                    QueueEntryStatus::Playing
                        | QueueEntryStatus::Ready
                        | QueueEntryStatus::PendingPreload
                ) && entry.canonical_provider_id == canon
            });
            if duplicate {
                return Err(format!(
                    "\"{}\" is already in the collaborative queue.",
                    track.title
                ));
            }
        }
        Ok(canon)
    }

    /// Enqueues a single track. For late guest contributions that are not yet verified cached,
    /// pass `initial_status = QueueEntryStatus::PendingPreload` so it stays out of the playable
    /// upcoming sequence until preloaded and verified.
    #[allow(clippy::too_many_arguments)]
    pub fn contribute_track(
        &mut self,
        contributor_id: &str,
        contributor_name: &str,
        role: ParticipantRole,
        track: Track,
        priority_override: bool,
        initial_status: QueueEntryStatus,
        explicit_entry_id: Option<String>,
    ) -> Result<SharedQueueEntry, String> {
        self.upsert_participant(contributor_id, contributor_name, role, true);
        let canon = self.validate_contribution(role, &track)?;

        let effective_override = role == ParticipantRole::Host && priority_override;
        let entry_id = explicit_entry_id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("qe-{}", Uuid::new_v4()));

        if self.entries.iter().any(|e| e.queue_entry_id == entry_id) {
            return self
                .entries
                .iter()
                .find(|e| e.queue_entry_id == entry_id)
                .cloned()
                .ok_or_else(|| "Duplicate queue entry id".to_string());
        }

        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);

        let mut entry = SharedQueueEntry {
            queue_entry_id: entry_id,
            track: track.clone(),
            canonical_provider_id: canon,
            contributor_id: contributor_id.to_string(),
            contributor_name: contributor_name.to_string(),
            role,
            status: initial_status,
            priority_override: effective_override,
            manual_position: None,
            committed: false,
            committed_rank: None,
            created_at_seq: seq,
        };
        entry.track = attach_entry_metadata(&track, &entry);

        if let Some(p) = self.participants.get_mut(contributor_id) {
            p.total_added = p.total_added.saturating_add(1);
        }

        self.entries.push(entry.clone());
        self.queue_revision = self.queue_revision.saturating_add(1);
        Ok(entry)
    }

    /// Promotes a `PendingPreload` contribution to `Ready` after audio download & verification,
    /// splicing it into the eligible upcoming playback sequence without replacing current/committed
    /// next track or reshuffling already committed imminent entries.
    pub fn mark_entry_ready(&mut self, queue_entry_id: &str) -> Result<bool, String> {
        let Some(pos) = self
            .entries
            .iter()
            .position(|e| e.queue_entry_id == queue_entry_id)
        else {
            return Err(format!("Unknown queue entry: {queue_entry_id}"));
        };

        if self.entries[pos].status == QueueEntryStatus::Ready {
            return Ok(false);
        }
        if self.entries[pos].status != QueueEntryStatus::PendingPreload {
            return Err("Only pending-preload entries can transition to ready".to_string());
        }

        self.entries[pos].status = QueueEntryStatus::Ready;
        self.splice_ready_entry_fairly(queue_entry_id);
        self.queue_revision = self.queue_revision.saturating_add(1);
        Ok(true)
    }

    /// Fails/cancels a `PendingPreload` entry so an unverified/failed download is never promoted
    /// into the playable sequence.
    pub fn discard_pending_preload(&mut self, queue_entry_id: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| {
            e.queue_entry_id == queue_entry_id && e.status == QueueEntryStatus::PendingPreload
        }) {
            entry.status = QueueEntryStatus::Removed;
            self.queue_revision = self.queue_revision.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Marks an active or queued entry as removed. If removed prior to playing by its contributor
    /// (anti-spam safeguard), adds `penalty_credits` so remove-and-readd cannot boost weight.
    pub fn remove_entry(&mut self, queue_entry_id: &str, apply_removal_penalty: bool) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.queue_entry_id == queue_entry_id)
        else {
            return false;
        };
        if matches!(
            entry.status,
            QueueEntryStatus::Played | QueueEntryStatus::Removed
        ) {
            return false;
        }
        let contributor_id = entry.contributor_id.clone();
        entry.status = QueueEntryStatus::Removed;
        entry.committed = false;
        entry.committed_rank = None;
        entry.manual_position = None;

        if apply_removal_penalty && let Some(p) = self.participants.get_mut(&contributor_id) {
            p.penalty_credits = (p.penalty_credits + 1.0).min(25.0);
        }

        self.queue_revision = self.queue_revision.saturating_add(1);
        self.recommit_next_if_missing();
        true
    }

    /// Host manual reorder in the upcoming playback sequence. Locks manual positions so manual
    /// host ordering takes precedence over automatic fairness for affected entries.
    pub fn host_reorder_upcoming(&mut self, from_index: usize, to_index: usize) -> bool {
        let mut upcoming = self.compute_upcoming_sequence();
        if from_index >= upcoming.len() || to_index >= upcoming.len() {
            return false;
        }
        if from_index == to_index {
            return true;
        }
        let moved = upcoming.remove(from_index);
        upcoming.insert(to_index, moved);

        let moved_id = upcoming[to_index].queue_entry_id.clone();
        // Assign explicit manual positions across upcoming so the host's chosen placement sticks.
        for (idx, item) in upcoming.iter().enumerate() {
            if let Some(entry) = self
                .entries
                .iter_mut()
                .find(|e| e.queue_entry_id == item.queue_entry_id)
            {
                entry.manual_position = Some(idx);
                if item.queue_entry_id == moved_id || idx <= to_index {
                    entry.committed = true;
                    entry.committed_rank = Some(idx as u64 + 1);
                }
            }
        }
        self.next_committed_rank = (upcoming.len() as u64).saturating_add(2);
        self.queue_revision = self.queue_revision.saturating_add(1);
        true
    }

    /// Advances playback to the next song in `UpcomingPlaybackSequence`: marks any previous Playing
    /// entry as `Played`, promotes `upcoming[0]` to `Playing`, decays removal penalties slightly as
    /// genuine playback progresses, and commits the new next track.
    pub fn advance_to_next(&mut self) -> Option<SharedQueueEntry> {
        // Retire currently playing entry if present.
        if let Some(prev) = self
            .entries
            .iter_mut()
            .find(|e| e.status == QueueEntryStatus::Playing)
        {
            prev.status = QueueEntryStatus::Played;
            prev.committed = false;
            prev.committed_rank = None;
            prev.manual_position = None;
            let contributor_id = prev.contributor_id.clone();
            if let Some(p) = self.participants.get_mut(&contributor_id) {
                p.total_played = p.total_played.saturating_add(1);
            }
        }

        // Decay removal penalty credits gradually during genuine playback progress.
        for participant in self.participants.values_mut() {
            if participant.penalty_credits > 0.0 {
                participant.penalty_credits = (participant.penalty_credits - 0.25).max(0.0);
            }
        }

        let upcoming = self.compute_upcoming_sequence();
        let next = upcoming.into_iter().next()?;

        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.queue_entry_id == next.queue_entry_id)
        {
            entry.status = QueueEntryStatus::Playing;
            entry.committed = false;
            entry.committed_rank = None;
            entry.manual_position = None;
            self.recent_played_contributors
                .push(entry.contributor_id.clone());
            if self.recent_played_contributors.len() > 16 {
                let drain_count = self.recent_played_contributors.len() - 16;
                self.recent_played_contributors.drain(0..drain_count);
            }
        }

        // Shift remaining manual positions down by 1 since slot 0 started playing.
        for entry in &mut self.entries {
            if entry.status == QueueEntryStatus::Ready
                && let Some(pos) = entry.manual_position
            {
                entry.manual_position = Some(pos.saturating_sub(1));
            }
        }

        self.commit_next_and_window(2);
        self.queue_revision = self.queue_revision.saturating_add(1);
        self.current_entry()
    }

    /// Commits at least the next upcoming track (`count >= 1`) so future contributions cannot
    /// displace the already committed next track.
    pub fn commit_next_and_window(&mut self, commit_count: usize) {
        if commit_count == 0 {
            return;
        }
        let upcoming = self.compute_upcoming_sequence();
        let mut assigned_ranks: HashSet<u64> = self
            .entries
            .iter()
            .filter_map(|e| {
                (e.status == QueueEntryStatus::Ready && e.committed).then_some(e.committed_rank?)
            })
            .collect();

        for item in upcoming.into_iter().take(commit_count) {
            if let Some(entry) = self
                .entries
                .iter_mut()
                .find(|e| e.queue_entry_id == item.queue_entry_id)
                && !entry.committed
            {
                while assigned_ranks.contains(&self.next_committed_rank) {
                    self.next_committed_rank = self.next_committed_rank.saturating_add(1);
                }
                let rank = self.next_committed_rank;
                self.next_committed_rank = self.next_committed_rank.saturating_add(1);
                assigned_ranks.insert(rank);
                entry.committed = true;
                entry.committed_rank = Some(rank);
            }
        }
    }

    fn recommit_next_if_missing(&mut self) {
        let has_committed = self
            .entries
            .iter()
            .any(|e| e.status == QueueEntryStatus::Ready && e.committed);
        if !has_committed {
            self.commit_next_and_window(1);
        }
    }

    /// Splices a newly ready entry into the committed upcoming sequence right after `committed[0]`
    /// if its fairness priority exceeds existing non-manually-pinned committed entries, preserving
    /// `committed[0]` and the relative order of all existing committed items.
    fn splice_ready_entry_fairly(&mut self, queue_entry_id: &str) {
        let mut ready: Vec<SharedQueueEntry> = self
            .entries
            .iter()
            .filter(|e| e.status == QueueEntryStatus::Ready)
            .cloned()
            .collect();

        let Some(new_idx_in_ready) = ready
            .iter()
            .position(|e| e.queue_entry_id == queue_entry_id)
        else {
            return;
        };
        let new_entry = ready.remove(new_idx_in_ready);

        // Build current ordered committed sequence (excluding manual_position reorders after index).
        let mut committed: Vec<SharedQueueEntry> =
            ready.into_iter().filter(|e| e.committed).collect();
        committed.sort_by_key(|e| {
            (
                e.manual_position.unwrap_or(usize::MAX),
                e.committed_rank.unwrap_or(u64::MAX),
                e.created_at_seq,
            )
        });

        if committed.is_empty() {
            return;
        }

        let new_pending = self.effective_pending_for(&new_entry.contributor_id);
        // Never replace index 0 (the committed next song). Start searching from index 1.
        let mut insert_at: Option<usize> = None;
        for idx in 1..=committed.len() {
            if idx == committed.len() {
                break;
            }
            let existing = &committed[idx];
            if existing.manual_position.is_some()
                || (existing.priority_override && !new_entry.priority_override)
            {
                continue;
            }
            let existing_pending = self.effective_pending_for(&existing.contributor_id);
            let better_priority =
                new_entry.priority_override || new_pending + 0.01 < existing_pending;
            if !better_priority {
                continue;
            }

            // Respect consecutive spacing against neighbor entries when alternatives exist.
            let prev_ok = committed
                .get(idx.saturating_sub(1))
                .map(|e| e.contributor_id != new_entry.contributor_id)
                .unwrap_or(true);
            let prev2_ok = if idx >= 2 {
                committed[idx - 2].contributor_id != new_entry.contributor_id
            } else {
                true
            };
            let next_ok = committed
                .get(idx)
                .map(|e| e.contributor_id != new_entry.contributor_id)
                .unwrap_or(true);
            if prev_ok && prev2_ok && next_ok {
                insert_at = Some(idx);
                break;
            }
        }

        if let Some(slot) = insert_at {
            committed.insert(slot, new_entry);
            // Re-assign monotonically increasing committed ranks preserving relative order.
            for (idx, item) in committed.iter().enumerate() {
                if let Some(entry) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.queue_entry_id == item.queue_entry_id)
                {
                    entry.committed = true;
                    entry.committed_rank = Some((idx as u64) + 1);
                }
            }
            self.next_committed_rank = (committed.len() as u64).saturating_add(2);
        }
    }

    fn effective_pending_for(&self, participant_id: &str) -> f64 {
        let count = self.pending_count_for(participant_id) as f64;
        let penalty = self
            .participants
            .get(participant_id)
            .map(|p| p.penalty_credits)
            .unwrap_or(0.0);
        count + penalty
    }

    pub fn current_entry(&self) -> Option<SharedQueueEntry> {
        self.entries
            .iter()
            .find(|e| e.status == QueueEntryStatus::Playing)
            .cloned()
    }

    pub fn shared_pool(&self) -> Vec<SharedQueueEntry> {
        self.entries
            .iter()
            .filter(|e| {
                matches!(
                    e.status,
                    QueueEntryStatus::PendingPreload | QueueEntryStatus::Ready
                )
            })
            .cloned()
            .collect()
    }

    pub fn pending_preload_by_priority(&self) -> Vec<SharedQueueEntry> {
        let mut pending: Vec<SharedQueueEntry> = self
            .entries
            .iter()
            .filter(|e| e.status == QueueEntryStatus::PendingPreload)
            .cloned()
            .collect();

        let alpha = self.config.alpha;
        pending.sort_by(|a, b| {
            b.priority_override
                .cmp(&a.priority_override)
                .then_with(|| {
                    let wa = participant_weight(
                        self.pending_count_for(&a.contributor_id),
                        self.participants
                            .get(&a.contributor_id)
                            .map(|p| p.penalty_credits)
                            .unwrap_or(0.0),
                        alpha,
                    );
                    let wb = participant_weight(
                        self.pending_count_for(&b.contributor_id),
                        self.participants
                            .get(&b.contributor_id)
                            .map(|p| p.penalty_credits)
                            .unwrap_or(0.0),
                        alpha,
                    );
                    wb.total_cmp(&wa)
                })
                .then_with(|| a.created_at_seq.cmp(&b.created_at_seq))
        });
        pending
    }

    /// Computes Tier 2: Upcoming Playback Sequence.
    /// Guarantees:
    /// - Never includes currently Playing or unverified PendingPreload items.
    /// - Preserves committed next item at index 0 and relative order of committed/manual entries.
    /// - Schedules remaining ready entries with weighted fairness + soft consecutive spacing.
    pub fn compute_upcoming_sequence(&self) -> Vec<SharedQueueEntry> {
        let ready: Vec<SharedQueueEntry> = self
            .entries
            .iter()
            .filter(|e| e.status == QueueEntryStatus::Ready)
            .cloned()
            .collect();
        if ready.is_empty() {
            return Vec::new();
        }

        // Partition ready items into fixed/committed/manual vs dynamic fair pool.
        let mut committed: Vec<SharedQueueEntry> = Vec::new();
        let mut overrides: Vec<SharedQueueEntry> = Vec::new();
        let mut pool_by_contributor: HashMap<String, Vec<SharedQueueEntry>> = HashMap::new();

        for item in ready {
            if item.committed || item.manual_position.is_some() {
                committed.push(item);
            } else if item.priority_override {
                overrides.push(item);
            } else {
                pool_by_contributor
                    .entry(item.contributor_id.clone())
                    .or_default()
                    .push(item);
            }
        }

        committed.sort_by_key(|e| {
            (
                e.manual_position.unwrap_or(usize::MAX),
                e.committed_rank.unwrap_or(u64::MAX),
                e.created_at_seq,
            )
        });
        overrides.sort_by_key(|e| e.created_at_seq);

        for list in pool_by_contributor.values_mut() {
            list.sort_by_key(|e| e.created_at_seq);
        }

        let mut sequence: Vec<SharedQueueEntry> = Vec::with_capacity(
            committed.len()
                + overrides.len()
                + pool_by_contributor.values().map(Vec::len).sum::<usize>(),
        );

        // Place committed[0] (the already committed next song) first if present.
        if !committed.is_empty() {
            sequence.push(committed.remove(0));
            // Explicit host priority overrides enter immediately after the committed next track.
            sequence.extend(overrides);
            sequence.extend(committed);
        } else {
            // No track committed yet: host priority overrides can take the front safely.
            sequence.extend(overrides);
        }

        // Track recent contributor history for consecutive spacing checks.
        let mut history: Vec<String> = self.recent_played_contributors.clone();
        for item in &sequence {
            history.push(item.contributor_id.clone());
        }

        // Seed RNG deterministically from session seed + fixed structural signature of current pool
        // in sorted sequence order so HashMap randomization never alters reconnect/sync determinism.
        let mut pool_seqs: Vec<u64> = pool_by_contributor
            .values()
            .flatten()
            .map(|e| e.created_at_seq)
            .collect();
        pool_seqs.sort_unstable();
        let pool_hash = pool_seqs.into_iter().fold(self.session_seed, |acc, seq| {
            acc.wrapping_mul(31).wrapping_add(seq)
        });
        let mut rng = DeterministicRng::new(pool_hash ^ self.session_seed);

        let preferred_gap = self.config.preferred_spacing;
        while !pool_by_contributor.is_empty() {
            let mut active_ids: Vec<String> = pool_by_contributor.keys().cloned().collect();
            active_ids.sort();

            let eligible_ids = select_eligible_contributors(&active_ids, &history, preferred_gap);

            // Compute fairness weights `1 / (1 + remaining_pending + penalty)^alpha`.
            let mut weighted: Vec<(String, f64)> = Vec::with_capacity(eligible_ids.len());
            let mut total_weight = 0.0_f64;
            for cid in &eligible_ids {
                let remaining = pool_by_contributor.get(cid).map(Vec::len).unwrap_or(0);
                let penalty = self
                    .participants
                    .get(cid)
                    .map(|p| p.penalty_credits)
                    .unwrap_or(0.0);
                let w = participant_weight(remaining, penalty, self.config.alpha);
                total_weight += w;
                weighted.push((cid.clone(), w));
            }

            let chosen_id = if total_weight <= 0.0 || weighted.len() == 1 {
                weighted[0].0.clone()
            } else {
                let mut threshold = rng.next_unit_f64() * total_weight;
                let mut picked = weighted[weighted.len() - 1].0.clone();
                for (cid, w) in &weighted {
                    if threshold <= *w {
                        picked = cid.clone();
                        break;
                    }
                    threshold -= *w;
                }
                picked
            };

            let list = pool_by_contributor
                .get_mut(&chosen_id)
                .expect("chosen contributor must exist in pool");
            let selected_entry = list.remove(0);
            if list.is_empty() {
                pool_by_contributor.remove(&chosen_id);
            }
            history.push(chosen_id);
            sequence.push(selected_entry);
        }

        sequence
    }

    /// Computes all 3 separated queue views plus participant weight telemetry.
    pub fn build_view(&self, preload_target: usize) -> ThreeTierQueueView {
        let upcoming_sequence = self.compute_upcoming_sequence();
        let preload_window = upcoming_sequence
            .iter()
            .take(preload_target.min(20))
            .cloned()
            .collect();
        let pending_preload = self.pending_preload_by_priority();

        let mut weights: Vec<ParticipantWeightSummary> = self
            .participants
            .values()
            .map(|p| {
                let pending = self.pending_count_for(&p.participant_id);
                ParticipantWeightSummary {
                    participant_id: p.participant_id.clone(),
                    display_name: p.display_name.clone(),
                    role: p.role,
                    is_connected: p.is_connected,
                    pending_songs: pending,
                    penalty_credits: p.penalty_credits,
                    weight: participant_weight(pending, p.penalty_credits, self.config.alpha),
                    total_added: p.total_added,
                    total_played: p.total_played,
                }
            })
            .collect();
        weights.sort_by(|a, b| {
            b.weight
                .total_cmp(&a.weight)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        ThreeTierQueueView {
            current_entry: self.current_entry(),
            shared_pool: self.shared_pool(),
            upcoming_sequence,
            preload_window,
            pending_preload,
            weights,
        }
    }
}

/// Applies soft spacing constraints:
/// 1. Prefer contributors who haven't appeared in the last `preferred_gap` tracks (`2` by default).
/// 2. Fall back to contributors different from the immediately previous track (`gap = 1`).
/// 3. Allow any contributor when only one contributor remains (`gap = 0`).
fn select_eligible_contributors(
    active_ids: &[String],
    history: &[String],
    preferred_gap: usize,
) -> Vec<String> {
    if active_ids.len() <= 1 || history.is_empty() {
        return active_ids.to_vec();
    }

    for gap in (1..=preferred_gap).rev() {
        let recent_window: Vec<&String> = history.iter().rev().take(gap).collect();
        let candidates: Vec<String> = active_ids
            .iter()
            .filter(|cid| !recent_window.contains(cid))
            .cloned()
            .collect();
        if !candidates.is_empty() {
            return candidates;
        }
    }

    active_ids.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yt_track(video_id: &str, title: &str) -> Track {
        Track {
            id: format!("ytmusic:{video_id}"),
            title: title.to_string(),
            artist: "Connect Artist".to_string(),
            album: "Connect Album".to_string(),
            album_artist: None,
            duration_ms: 180_000,
            artwork_uri: None,
            source_kind: "provider".to_string(),
            provider_id: Some("youtube_music".to_string()),
            file_uri: None,
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            is_liked: false,
            play_count: 0,
            date_added: String::new(),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    #[test]
    fn fairness_weight_prefers_fewer_pending_songs_and_recovers_as_tracks_play() {
        let w1 = participant_weight(1, 0.0, 1.0);
        let w5 = participant_weight(5, 0.0, 1.0);
        let w20 = participant_weight(20, 0.0, 1.0);
        assert!((w1 - 0.5).abs() < 1e-9);
        assert!(w1 > w5);
        assert!(w5 > w20);
        assert!(w20 > 0.0, "heavy contributors must remain nonzero weight");

        // Higher alpha strengthens preference for underserved contributors.
        let ratio_alpha1 = participant_weight(0, 0.0, 1.0) / participant_weight(10, 0.0, 1.0);
        let ratio_alpha2 = participant_weight(0, 0.0, 2.0) / participant_weight(10, 0.0, 2.0);
        assert!(ratio_alpha2 > ratio_alpha1);
    }

    #[test]
    fn distribution_avoids_monopoly_without_pushing_all_to_end() {
        let mut engine = FairQueueEngine::new(FairnessConfig::default(), 42);
        // Alice floods 10 songs; Bob, Carol, and Dan each contribute 4 songs (12 non-Alice songs).
        for i in 0..10 {
            engine
                .contribute_track(
                    "alice",
                    "Alice",
                    ParticipantRole::Guest,
                    yt_track(&format!("vidA{i}"), &format!("Alice {i}")),
                    false,
                    QueueEntryStatus::Ready,
                    None,
                )
                .unwrap();
        }
        for (pid, name) in [("bob", "Bob"), ("carol", "Carol"), ("dan", "Dan")] {
            for j in 0..4 {
                engine
                    .contribute_track(
                        pid,
                        name,
                        ParticipantRole::Guest,
                        yt_track(&format!("vid_{pid}_{j}"), &format!("{name} {j}")),
                        false,
                        QueueEntryStatus::Ready,
                        None,
                    )
                    .unwrap();
            }
        }

        let seq = engine.compute_upcoming_sequence();
        assert_eq!(seq.len(), 22);

        // Low-pending contributors dominate early positions, while Alice mixes into the stream
        // rather than having all 10 tracks pushed strictly after index 11.
        let first_alice_pos = seq
            .iter()
            .position(|e| e.contributor_id == "alice")
            .expect("Alice has tracks");
        let non_alice_last_idx = seq
            .iter()
            .rposition(|e| e.contributor_id != "alice")
            .unwrap();
        let early_non_alice = seq[..8]
            .iter()
            .filter(|e| e.contributor_id != "alice")
            .count();
        assert!(
            early_non_alice >= 5,
            "Lower-count contributors should get higher early share (got {early_non_alice}/8)"
        );
        assert!(
            first_alice_pos < non_alice_last_idx,
            "Heavy contributor must mix into sequence before all others finish (first Alice={first_alice_pos}, last non-Alice={non_alice_last_idx})"
        );

        // No consecutive same-contributor runs while >= 2 contributors still have songs.
        let tail_start = seq
            .iter()
            .position(|e| {
                seq[seq
                    .iter()
                    .position(|x| x.queue_entry_id == e.queue_entry_id)
                    .unwrap()..]
                    .iter()
                    .all(|rem| rem.contributor_id == e.contributor_id)
            })
            .unwrap_or(seq.len());
        for w in seq[..tail_start].windows(2) {
            assert_ne!(
                w[0].contributor_id, w[1].contributor_id,
                "Consecutive same-participant tracks should be avoided while alternatives exist"
            );
        }
    }

    #[test]
    fn single_contributor_and_soft_spacing_relax_cleanly() {
        let mut engine = FairQueueEngine::new(FairnessConfig::default(), 7);
        for i in 0..4 {
            engine
                .contribute_track(
                    "solo",
                    "Solo",
                    ParticipantRole::Host,
                    yt_track(&format!("solo{i}"), &format!("Solo {i}")),
                    false,
                    QueueEntryStatus::Ready,
                    None,
                )
                .unwrap();
        }
        let seq = engine.compute_upcoming_sequence();
        assert_eq!(seq.len(), 4);
    }

    #[test]
    fn guest_duplicate_canonical_id_rejected_while_host_duplicate_allowed() {
        let mut engine = FairQueueEngine::default();
        let entry1 = engine
            .contribute_track(
                "guest1",
                "Guest 1",
                ParticipantRole::Guest,
                yt_track("dQw4w9WgXcQ?si=abc", "Song One"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();

        let err = engine
            .contribute_track(
                "guest2",
                "Guest 2",
                ParticipantRole::Guest,
                yt_track("dQw4w9WgXcQ", "Song One Again"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .expect_err("Guest duplicate canonical ID must be rejected");
        assert!(err.contains("already in the collaborative queue"));

        // Host may intentionally duplicate with a distinct queue_entry_id.
        let host_dup = engine
            .contribute_track(
                "host",
                "Host",
                ParticipantRole::Host,
                yt_track("dQw4w9WgXcQ", "Song One Encore"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();
        assert_ne!(entry1.queue_entry_id, host_dup.queue_entry_id);
        assert_eq!(entry1.canonical_provider_id, host_dup.canonical_provider_id);
    }

    #[test]
    fn remove_readd_and_disconnect_reconnect_cannot_reset_penalty() {
        let mut engine = FairQueueEngine::default();
        let a1 = engine
            .contribute_track(
                "spamer",
                "Spammer",
                ParticipantRole::Guest,
                yt_track("s1", "Spam 1"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();
        let _a2 = engine
            .contribute_track(
                "spamer",
                "Spammer",
                ParticipantRole::Guest,
                yt_track("s2", "Spam 2"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();

        engine.remove_entry(&a1.queue_entry_id, true);
        engine.set_participant_connected("spamer", false);
        engine.set_participant_connected("spamer", true);

        engine
            .contribute_track(
                "spamer",
                "Spammer",
                ParticipantRole::Guest,
                yt_track("s3", "Spam 3"),
                false,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();

        let view = engine.build_view(10);
        let spammer_summary = view
            .weights
            .iter()
            .find(|w| w.participant_id == "spamer")
            .unwrap();
        assert!(
            spammer_summary.penalty_credits >= 1.0,
            "Penalty must survive removal + disconnect/reconnect"
        );
        assert!(
            spammer_summary.weight < participant_weight(1, 0.0, 1.0),
            "Removed + re-added track must have lower weight than an honest 1-song contributor"
        );
    }

    #[test]
    fn committed_next_and_late_verified_contribution_insertion_preserve_stability() {
        let mut engine = FairQueueEngine::new(FairnessConfig::default(), 99);
        for i in 0..10 {
            engine
                .contribute_track(
                    "heavy",
                    "Heavy User",
                    ParticipantRole::Guest,
                    yt_track(&format!("h{i}"), &format!("Heavy {i}")),
                    false,
                    QueueEntryStatus::Ready,
                    None,
                )
                .unwrap();
        }
        // Commit initial 10 upcoming tracks (like an already-preloaded 10-track window).
        engine.commit_next_and_window(10);
        let before = engine.compute_upcoming_sequence();
        assert_eq!(before.len(), 10);
        let committed_next_id = before[0].queue_entry_id.clone();

        // New participant contributes a track while 10 tracks are already preloaded -> starts PendingPreload.
        let late = engine
            .contribute_track(
                "newcomer",
                "Newcomer",
                ParticipantRole::Guest,
                yt_track("late1", "Late Hit"),
                false,
                QueueEntryStatus::PendingPreload,
                None,
            )
            .unwrap();

        // Must NOT enter upcoming sequence before verification.
        let before_verify = engine.compute_upcoming_sequence();
        assert!(
            !before_verify
                .iter()
                .any(|e| e.queue_entry_id == late.queue_entry_id),
            "PendingPreload contribution must wait until downloaded & verified"
        );

        // Mark verified -> splices into eligible slot after committed[0], preserving relative order of rest.
        engine.mark_entry_ready(&late.queue_entry_id).unwrap();
        let after_verify = engine.compute_upcoming_sequence();
        assert_eq!(after_verify.len(), 11);
        assert_eq!(
            after_verify[0].queue_entry_id, committed_next_id,
            "Already committed next track must NEVER be displaced"
        );
        let late_pos = after_verify
            .iter()
            .position(|e| e.queue_entry_id == late.queue_entry_id)
            .unwrap();
        assert!(
            (1..10).contains(&late_pos),
            "High-fairness verified late contribution must enter preload window (pos={late_pos})"
        );
    }

    #[test]
    fn normal_host_tracks_follow_fairness_unless_priority_override_or_manual_reorder() {
        let mut engine = FairQueueEngine::default();
        // Host adds 3 normal tracks first; guest adds 1.
        for i in 0..3 {
            engine
                .contribute_track(
                    "host",
                    "Host",
                    ParticipantRole::Host,
                    yt_track(&format!("host{i}"), &format!("Host {i}")),
                    false,
                    QueueEntryStatus::Ready,
                    None,
                )
                .unwrap();
        }
        let view_before = engine.build_view(10);
        let host_w = view_before
            .weights
            .iter()
            .find(|w| w.participant_id == "host")
            .unwrap()
            .weight;
        assert!(
            (host_w - participant_weight(3, 0.0, 1.0)).abs() < 1e-9,
            "Normal host tracks count toward fairness weight just like guests"
        );

        // Lock slot 0 as committed next.
        engine.commit_next_and_window(1);
        let seq_before = engine.compute_upcoming_sequence();
        let locked_next = seq_before[0].queue_entry_id.clone();

        // Explicit host priority override inserts immediately after committed next track (slot 1).
        let urgent = engine
            .contribute_track(
                "host",
                "Host",
                ParticipantRole::Host,
                yt_track("urgent", "Host Urgent"),
                true,
                QueueEntryStatus::Ready,
                None,
            )
            .unwrap();
        let seq_after = engine.compute_upcoming_sequence();
        assert_eq!(seq_after[0].queue_entry_id, locked_next);
        assert_eq!(seq_after[1].queue_entry_id, urgent.queue_entry_id);

        // Manual host reorder takes precedence and persists.
        assert!(engine.host_reorder_upcoming(3, 1));
        let moved_target = seq_after[3].queue_entry_id.clone();
        let seq_reordered = engine.compute_upcoming_sequence();
        assert_eq!(seq_reordered[1].queue_entry_id, moved_target);
    }
}
