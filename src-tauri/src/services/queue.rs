use crate::domain::Track;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl Default for RepeatMode {
    fn default() -> Self {
        Self::Off
    }
}

impl From<&str> for RepeatMode {
    fn from(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "one" => Self::One,
            _ => Self::Off,
        }
    }
}

impl RepeatMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueManager {
    tracks: Vec<Track>,
    current_index: Option<usize>,
    repeat_mode: RepeatMode,
    shuffle_enabled: bool,
    shuffle_order: Vec<usize>,
    shuffle_pos: usize,
}

impl Default for QueueManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            shuffle_order: Vec::new(),
            shuffle_pos: 0,
        }
    }

    pub fn set_queue(&mut self, tracks: Vec<Track>, start_track_id: Option<&str>) {
        let initial_index = if let Some(id) = start_track_id {
            tracks.iter().position(|t| t.id == id).unwrap_or(0)
        } else {
            0
        };

        let has_items = !tracks.is_empty();
        self.tracks = tracks;
        self.current_index = if has_items { Some(initial_index) } else { None };

        if self.shuffle_enabled {
            self.rebuild_shuffle(self.current_index);
        } else {
            self.shuffle_order.clear();
            self.shuffle_pos = 0;
        }
    }

    pub fn add_track(&mut self, track: Track) {
        let new_idx = self.tracks.len();
        self.tracks.push(track);
        if self.current_index.is_none() {
            self.current_index = Some(new_idx);
        }
        if self.shuffle_enabled {
            self.shuffle_order.push(new_idx);
        }
    }

    pub fn add_tracks(&mut self, tracks: Vec<Track>) {
        for track in tracks {
            self.add_track(track);
        }
    }

    pub fn remove_at(&mut self, pos: usize) -> Option<Track> {
        if pos >= self.tracks.len() {
            return None;
        }
        let removed = self.tracks.remove(pos);

        // Adjust current_index
        if let Some(curr) = self.current_index {
            if curr == pos {
                if self.tracks.is_empty() {
                    self.current_index = None;
                } else if curr >= self.tracks.len() {
                    self.current_index = Some(self.tracks.len() - 1);
                }
            } else if curr > pos {
                self.current_index = Some(curr - 1);
            }
        }

        if self.shuffle_enabled {
            // Preserve shuffle history: remove the deleted index from the
            // permutation and shift the remaining indices instead of
            // regenerating the whole order.
            let removed_order_pos = self.shuffle_order.iter().position(|&i| i == pos);
            self.shuffle_order.retain(|&i| i != pos);
            for index in &mut self.shuffle_order {
                if *index > pos {
                    *index -= 1;
                }
            }
            if removed_order_pos.is_none() && self.shuffle_pos > 0 {
                // Removing an entry before the cursor shifts it left.
                if self
                    .shuffle_order
                    .get(self.shuffle_pos)
                    .is_some_and(|&idx| idx < pos)
                {
                    self.shuffle_pos -= 1;
                }
            }
            if let Some(curr) = self.current_index {
                if let Some(order_pos) = self.shuffle_order.iter().position(|&i| i == curr) {
                    self.shuffle_pos = order_pos;
                }
            }
            if self.shuffle_order.is_empty() {
                self.shuffle_pos = 0;
            } else if self.shuffle_pos >= self.shuffle_order.len() {
                self.shuffle_pos = self.shuffle_order.len() - 1;
            }
        }

        Some(removed)
    }

    pub fn remove_track(&mut self, track_id: &str) -> Option<Track> {
        let pos = self.tracks.iter().position(|t| t.id == track_id)?;
        self.remove_at(pos)
    }

    pub fn find_track_by_id(&self, track_id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == track_id)
    }

    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tracks.len() || to >= self.tracks.len() || from == to {
            return false;
        }

        let item = self.tracks.remove(from);
        self.tracks.insert(to, item);

        // Adjust current_index pointer
        if let Some(curr) = self.current_index {
            if curr == from {
                self.current_index = Some(to);
            } else if from < curr && to >= curr {
                self.current_index = Some(curr - 1);
            } else if from > curr && to <= curr {
                self.current_index = Some(curr + 1);
            }
        }

        if self.shuffle_enabled {
            // Remap the existing permutation instead of regenerating it, so
            // the played/unplayed history is preserved across reordering.
            for index in &mut self.shuffle_order {
                if *index == from {
                    *index = to;
                } else if from < to && *index > from && *index <= to {
                    *index -= 1;
                } else if to < from && *index >= to && *index < from {
                    *index += 1;
                }
            }
            if let Some(curr) = self.current_index {
                if let Some(order_pos) = self.shuffle_order.iter().position(|&i| i == curr) {
                    self.shuffle_pos = order_pos;
                }
            }
        }
        true
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
        self.shuffle_order.clear();
        self.shuffle_pos = 0;
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.tracks.get(idx))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn set_current_index(&mut self, index: usize) -> Option<&Track> {
        if index < self.tracks.len() {
            self.current_index = Some(index);
            if self.shuffle_enabled {
                if let Some(pos) = self.shuffle_order.iter().position(|&i| i == index) {
                    self.shuffle_pos = pos;
                }
            }
            self.current_track()
        } else {
            None
        }
    }

    pub fn select_track_by_id(&mut self, track_id: &str) -> Option<&Track> {
        if let Some(pos) = self.tracks.iter().position(|t| t.id == track_id) {
            self.set_current_index(pos)
        } else {
            None
        }
    }

    pub fn peek_next(&mut self) -> Option<(usize, &Track)> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.repeat_mode == RepeatMode::One {
            return self
                .current_index
                .and_then(|idx| self.tracks.get(idx).map(|t| (idx, t)));
        }

        if self.shuffle_enabled && !self.shuffle_order.is_empty() {
            if self.shuffle_pos + 1 < self.shuffle_order.len() {
                let next_idx = self.shuffle_order[self.shuffle_pos + 1];
                self.tracks.get(next_idx).map(|t| (next_idx, t))
            } else if self.repeat_mode == RepeatMode::All {
                // Begin a fresh shuffle cycle instead of replaying the
                // permutation pinned when shuffle was originally enabled.
                let last_played = self.current_index;
                self.rebuild_shuffle(None);
                let start_pos =
                    if self.shuffle_order.len() > 1 && Some(self.shuffle_order[0]) == last_played {
                        1
                    } else {
                        0
                    };
                self.shuffle_pos = start_pos;
                let next_idx = self.shuffle_order[start_pos];
                self.tracks.get(next_idx).map(|t| (next_idx, t))
            } else {
                None
            }
        } else {
            match self.current_index {
                Some(curr) => {
                    if curr + 1 < self.tracks.len() {
                        let next_idx = curr + 1;
                        self.tracks.get(next_idx).map(|t| (next_idx, t))
                    } else if self.repeat_mode == RepeatMode::All {
                        self.tracks.get(0).map(|t| (0, t))
                    } else {
                        None
                    }
                }
                None => self.tracks.get(0).map(|t| (0, t)),
            }
        }
    }

    pub fn peek_previous(&self, position_ms: u64) -> Option<(usize, &Track)> {
        if self.tracks.is_empty() {
            return None;
        }

        if position_ms > 3000 || self.repeat_mode == RepeatMode::One {
            return self
                .current_index
                .and_then(|idx| self.tracks.get(idx).map(|t| (idx, t)));
        }

        if self.shuffle_enabled && !self.shuffle_order.is_empty() {
            if self.shuffle_pos > 0 {
                let prev_idx = self.shuffle_order[self.shuffle_pos - 1];
                self.tracks.get(prev_idx).map(|t| (prev_idx, t))
            } else if self.repeat_mode == RepeatMode::All {
                let prev_idx = self.shuffle_order[self.shuffle_order.len() - 1];
                self.tracks.get(prev_idx).map(|t| (prev_idx, t))
            } else {
                self.current_index
                    .and_then(|idx| self.tracks.get(idx).map(|t| (idx, t)))
            }
        } else {
            match self.current_index {
                Some(curr) => {
                    if curr > 0 {
                        let prev_idx = curr - 1;
                        self.tracks.get(prev_idx).map(|t| (prev_idx, t))
                    } else if self.repeat_mode == RepeatMode::All {
                        let prev_idx = self.tracks.len() - 1;
                        self.tracks.get(prev_idx).map(|t| (prev_idx, t))
                    } else {
                        self.current_index
                            .and_then(|idx| self.tracks.get(idx).map(|t| (idx, t)))
                    }
                }
                None => self.tracks.get(0).map(|t| (0, t)),
            }
        }
    }

    pub fn next(&mut self) -> Option<&Track> {
        let (next_idx, _) = self.peek_next()?;
        self.set_current_index(next_idx)
    }

    pub fn previous(&mut self, position_ms: u64) -> Option<&Track> {
        let (prev_idx, _) = self.peek_previous(position_ms)?;
        self.set_current_index(prev_idx)
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle_enabled == enabled {
            return;
        }
        self.shuffle_enabled = enabled;
        if enabled {
            self.rebuild_shuffle(self.current_index);
        } else {
            self.shuffle_order.clear();
            self.shuffle_pos = 0;
        }
    }

    pub fn is_shuffle(&self) -> bool {
        self.shuffle_enabled
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    fn rebuild_shuffle(&mut self, pinned_index: Option<usize>) {
        let n = self.tracks.len();
        if n == 0 {
            self.shuffle_order.clear();
            self.shuffle_pos = 0;
            return;
        }

        let mut indices: Vec<usize> = (0..n).collect();
        // Seeded pseudo-random shuffle with session/time entropy, keeping current track pinned
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        let mut state: u64 = seed ^ (n as u64);
        for i in (1..n).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = ((state >> 33) as usize) % (i + 1);
            indices.swap(i, j);
        }

        // Keep pinned current track at index 0 of the shuffle list
        if let Some(pinned) = pinned_index {
            if let Some(found_pos) = indices.iter().position(|&idx| idx == pinned) {
                indices.swap(0, found_pos);
            }
        }

        self.shuffle_order = indices;
        self.shuffle_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_track(id: &str, title: &str) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            album_artist: None,
            duration_ms: 180_000,
            artwork_uri: None,
            source_kind: "local".to_string(),
            provider_id: None,
            file_uri: Some(format!("C:/music/{id}.flac")),
            release_year: Some(2024),
            track_number: Some(1),
            disc_number: Some(1),
            genre: Some("Ambient".to_string()),
            is_liked: false,
            play_count: 0,
            date_added: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_queue_sequential_advance_and_stop() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
            ],
            None,
        );

        assert_eq!(q.current_track().unwrap().id, "1");
        assert_eq!(q.next().unwrap().id, "2");
        assert_eq!(q.next().unwrap().id, "3");
        assert!(q.next().is_none()); // End of queue with repeat off
    }

    #[test]
    fn test_queue_repeat_all() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![dummy_track("1", "Track 1"), dummy_track("2", "Track 2")],
            None,
        );
        q.set_repeat_mode(RepeatMode::All);

        assert_eq!(q.current_track().unwrap().id, "1");
        assert_eq!(q.next().unwrap().id, "2");
        assert_eq!(q.next().unwrap().id, "1"); // Wrapped around
    }

    #[test]
    fn test_queue_repeat_one() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![dummy_track("1", "Track 1"), dummy_track("2", "Track 2")],
            None,
        );
        q.set_repeat_mode(RepeatMode::One);

        assert_eq!(q.next().unwrap().id, "1");
        assert_eq!(q.next().unwrap().id, "1");
    }

    #[test]
    fn test_queue_previous_restart_logic() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![dummy_track("1", "Track 1"), dummy_track("2", "Track 2")],
            Some("2"),
        );

        // If played > 3000ms, previous restarts current track
        assert_eq!(q.previous(5000).unwrap().id, "2");

        // If played <= 3000ms, moves to previous track
        assert_eq!(q.previous(1000).unwrap().id, "1");
    }

    #[test]
    fn test_queue_shuffle_preserves_current_track() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
                dummy_track("4", "Track 4"),
            ],
            Some("3"),
        );

        q.set_shuffle(true);
        // Current track remains active at start of shuffle
        assert_eq!(q.current_track().unwrap().id, "3");
        assert_eq!(q.shuffle_order.len(), 4);
        assert_eq!(q.shuffle_order[0], 2); // index 2 corresponds to Track 3
    }

    #[test]
    fn test_queue_remove_and_reorder() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
            ],
            Some("2"),
        );

        assert_eq!(q.current_track().unwrap().id, "2");

        // Reorder track 0 ("1") to position 2
        assert!(q.reorder(0, 2));
        assert_eq!(q.tracks[2].id, "1");
        // Current index should adjust to 0 ("2")
        assert_eq!(q.current_track().unwrap().id, "2");

        // Remove track 3
        let removed = q.remove_track("3");
        assert_eq!(removed.unwrap().id, "3");
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn test_queue_duplicate_entries_and_remove_at() {
        let mut q = QueueManager::new();
        // Add two instances of Track "1" and one of Track "2"
        q.set_queue(
            vec![
                dummy_track("1", "Track 1 - copy A"),
                dummy_track("2", "Track 2"),
                dummy_track("1", "Track 1 - copy B"),
            ],
            Some("1"),
        );

        assert_eq!(q.len(), 3);
        assert_eq!(q.current_index(), Some(0));

        // Remove specifically the second instance of Track 1 at index 2
        let removed = q.remove_at(2);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().title, "Track 1 - copy B");
        assert_eq!(q.len(), 2);
        // Current index remains 0
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn test_queue_remove_current_track_adjusts_index() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
            ],
            Some("2"), // current_index is 1
        );

        // Remove active track at index 1
        let removed = q.remove_at(1);
        assert_eq!(removed.unwrap().id, "2");
        assert_eq!(q.len(), 2);
        // Track 3 has now shifted to index 1, which becomes the current track
        assert_eq!(q.current_index(), Some(1));
        assert_eq!(q.current_track().unwrap().id, "3");

        // Remove last track
        let removed_last = q.remove_at(1);
        assert_eq!(removed_last.unwrap().id, "3");
        // Current index clamps to 0 ("1")
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current_track().unwrap().id, "1");

        // Remove final track
        let _ = q.remove_at(0);
        assert_eq!(q.current_index(), None);
        assert!(q.current_track().is_none());
    }

    #[test]
    fn test_shuffle_history_preserved_on_reorder_and_remove() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
                dummy_track("4", "Track 4"),
                dummy_track("5", "Track 5"),
            ],
            Some("3"),
        );
        q.set_shuffle(true);
        q.next();
        q.next();

        let current_id = q.current_track().unwrap().id.clone();
        let order_before: Vec<String> = q
            .shuffle_order
            .iter()
            .map(|&i| q.tracks[i].id.clone())
            .collect();

        assert!(q.reorder(0, 4));
        let order_after: Vec<String> = q
            .shuffle_order
            .iter()
            .map(|&i| q.tracks[i].id.clone())
            .collect();
        assert_eq!(order_before, order_after);
        assert_eq!(q.current_track().unwrap().id, current_id);

        // Removing a non-current track must leave the relative shuffle
        // history (played/unplayed order) intact.
        let remove_idx = (0..q.len())
            .find(|&i| q.tracks[i].id != current_id)
            .unwrap();
        let removed_id = q.tracks[remove_idx].id.clone();
        let order_before_remove: Vec<String> = q
            .shuffle_order
            .iter()
            .map(|&i| q.tracks[i].id.clone())
            .collect();
        q.remove_at(remove_idx);
        let order_after_remove: Vec<String> = q
            .shuffle_order
            .iter()
            .map(|&i| q.tracks[i].id.clone())
            .collect();
        let expected: Vec<String> = order_before_remove
            .into_iter()
            .filter(|id| id != &removed_id)
            .collect();
        assert_eq!(order_after_remove, expected);
        assert_eq!(q.current_track().unwrap().id, current_id);
    }

    #[test]
    fn test_shuffle_repeat_all_starts_new_cycle() {
        let mut q = QueueManager::new();
        q.set_queue(
            vec![
                dummy_track("1", "Track 1"),
                dummy_track("2", "Track 2"),
                dummy_track("3", "Track 3"),
                dummy_track("4", "Track 4"),
            ],
            Some("2"),
        );
        q.set_shuffle(true);
        q.set_repeat_mode(RepeatMode::All);

        for _ in 0..3 {
            q.next();
        }
        let last = q.current_track().unwrap().id.clone();
        let wrapped = q.next().expect("repeat-all should wrap").id.clone();
        assert_ne!(wrapped, last);

        let mut indices = q.shuffle_order.clone();
        indices.sort_unstable();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }
}
