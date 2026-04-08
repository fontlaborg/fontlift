//! Crash-recovery journal for multi-step font operations.
//!
//! Installing or removing a font usually involves more than one step, such as
//! copy, register, unregister, delete, or cache clearing. If `fontlift` stops
//! halfway through, the journal preserves what was planned and how far the work
//! got.
//!
//! `fontlift doctor` reads incomplete entries and asks recovery code to resume
//! the remaining steps. The current built-in flow mostly rolls work forward or
//! skips steps that already happened.
//!
//! ## How it works
//!
//! 1. Call [`Journal::record_operation`] with the list of [`JournalAction`]s
//!    planned for an install or removal. This writes the entry to disk.
//! 2. Execute each action. After each one succeeds, call [`Journal::mark_step`]
//!    so the journal knows how far you got.
//! 3. When all actions are done, call [`Journal::mark_completed`].
//! 4. If something crashes, the entry stays incomplete. On the next startup,
//!    [`recover_incomplete_operations`] finds it and resumes the remaining
//!    steps according to [`RecoveryPolicy`].
//!
//! ## Running recovery
//!
//! ```text
//! fontlift doctor
//! ```
//!
//! That command calls [`recover_incomplete_operations`] and reports what it
//! found and what recovery succeeded.
//!
//! ## Journal file location
//!
//! | Platform | Default path |
//! |---|---|
//! | macOS | `~/Library/Application Support/FontLift/journal.json` |
//! | Windows | `%LOCALAPPDATA%\FontLift\journal.json` |
//! | Linux / other | `~/.local/share/fontlift/journal.json` |
//!
//! Override with `FONTLIFT_JOURNAL_PATH`, which is especially handy in tests.
//!
//! ## Atomic writes
//!
//! The journal is always written to a `.tmp` file first, then renamed into
//! place. Within one filesystem, that rename is atomic, so readers see either
//! the old journal or the new one, never a half-written mix.

use crate::{FontError, FontResult, FontScope};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// One recoverable step recorded in the journal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JournalAction {
    CopyFile { from: PathBuf, to: PathBuf },
    RegisterFont { path: PathBuf, scope: FontScope },
    UnregisterFont { path: PathBuf, scope: FontScope },
    DeleteFile { path: PathBuf },
    ClearCache { scope: FontScope },
}

impl JournalAction {
    pub fn description(&self) -> String {
        match self {
            JournalAction::CopyFile { from, to } => {
                format!("Copy {} to {}", from.display(), to.display())
            }
            JournalAction::RegisterFont { path, scope } => {
                format!("Register {} ({:?})", path.display(), scope)
            }
            JournalAction::UnregisterFont { path, scope } => {
                format!("Unregister {} ({:?})", path.display(), scope)
            }
            JournalAction::DeleteFile { path } => {
                format!("Delete {}", path.display())
            }
            JournalAction::ClearCache { scope } => {
                format!("Clear caches ({:?})", scope)
            }
        }
    }
}

/// Recorded state for one multi-step operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    #[serde(with = "systemtime_serde")]
    pub started_at: SystemTime,
    pub completed: bool,
    pub actions: Vec<JournalAction>,
    /// Index of the next action to attempt.
    ///
    /// `0` means nothing has finished yet. `actions.len()` means every action
    /// has finished.
    pub current_step: usize,
    pub description: Option<String>,
}

impl JournalEntry {
    pub fn new(actions: Vec<JournalAction>, description: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: SystemTime::now(),
            completed: false,
            actions,
            current_step: 0,
            description,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        !self.completed && !self.actions.is_empty()
    }

    pub fn current_action(&self) -> Option<&JournalAction> {
        self.actions.get(self.current_step)
    }

    pub fn remaining_actions(&self) -> &[JournalAction] {
        if self.current_step < self.actions.len() {
            &self.actions[self.current_step..]
        } else {
            &[]
        }
    }
}

/// Serde helpers for `SystemTime`.
mod systemtime_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    pub entries: Vec<JournalEntry>,
}

impl Journal {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn record_operation(
        &mut self,
        actions: Vec<JournalAction>,
        description: Option<String>,
    ) -> Uuid {
        let entry = JournalEntry::new(actions, description);
        let id = entry.id;
        self.entries.push(entry);
        id
    }

    pub fn find_entry(&self, id: Uuid) -> Option<&JournalEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn find_entry_mut(&mut self, id: Uuid) -> Option<&mut JournalEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn mark_step(&mut self, id: Uuid, step: usize) -> FontResult<()> {
        let entry = self
            .find_entry_mut(id)
            .ok_or_else(|| FontError::InvalidFormat(format!("Journal entry not found: {id}")))?;
        entry.current_step = step;
        Ok(())
    }

    pub fn mark_completed(&mut self, id: Uuid) -> FontResult<()> {
        let entry = self
            .find_entry_mut(id)
            .ok_or_else(|| FontError::InvalidFormat(format!("Journal entry not found: {id}")))?;
        entry.completed = true;
        Ok(())
    }

    pub fn incomplete_entries(&self) -> Vec<&JournalEntry> {
        self.entries.iter().filter(|e| e.is_incomplete()).collect()
    }

    pub fn cleanup_old_entries(&mut self, max_age_secs: u64) {
        let now = SystemTime::now();
        self.entries.retain(|e| {
            if !e.completed {
                return true; // Keep incomplete entries
            }
            match now.duration_since(e.started_at) {
                Ok(age) => age.as_secs() < max_age_secs,
                Err(_) => true, // Keep if time comparison fails
            }
        });
    }
}

/// Return the journal path for the current platform.
///
/// `FONTLIFT_JOURNAL_PATH` overrides the normal location. Test code can also
/// redirect the journal via `FONTLIFT_FAKE_REGISTRY_ROOT`.
pub fn journal_path() -> PathBuf {
    // Check for override (useful for testing)
    if let Ok(override_path) = std::env::var("FONTLIFT_JOURNAL_PATH") {
        return PathBuf::from(override_path);
    }

    // Check for fake registry root (testing mode)
    if let Ok(root) = std::env::var("FONTLIFT_FAKE_REGISTRY_ROOT") {
        return PathBuf::from(root).join("journal.json");
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("FontLift")
            .join("journal.json")
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
            .join("FontLift")
            .join("journal.json")
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("fontlift")
            .join("journal.json")
    }
}

/// Load the journal from disk.
///
/// Missing files are treated as an empty journal.
pub fn load_journal() -> FontResult<Journal> {
    let path = journal_path();
    if !path.exists() {
        return Ok(Journal::new());
    }

    let content = fs::read_to_string(&path).map_err(|e| {
        FontError::IoError(std::io::Error::new(
            e.kind(),
            format!("Failed to read journal: {e}"),
        ))
    })?;

    serde_json::from_str(&content)
        .map_err(|e| FontError::InvalidFormat(format!("Failed to parse journal: {e}")))
}

/// Save the journal with a temp-file-then-rename write.
pub fn save_journal(journal: &Journal) -> FontResult<()> {
    let path = journal_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(FontError::IoError)?;
    }

    // Write to temp file first
    let temp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(journal)
        .map_err(|e| FontError::InvalidFormat(format!("Failed to serialize journal: {e}")))?;

    fs::write(&temp_path, &content).map_err(|e| {
        FontError::IoError(std::io::Error::new(
            e.kind(),
            format!("Failed to write journal temp file: {e}"),
        ))
    })?;

    // Atomic rename
    fs::rename(&temp_path, &path).map_err(|e| {
        FontError::IoError(std::io::Error::new(
            e.kind(),
            format!("Failed to rename journal file: {e}"),
        ))
    })?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPolicy {
    RollForward,
    RollBack,
    Skip,
}

#[derive(Debug)]
pub struct ActionRecoveryResult {
    pub action: JournalAction,
    pub policy: RecoveryPolicy,
    pub success: bool,
    pub message: Option<String>,
}

/// Recover incomplete operations.
///
/// For each incomplete entry, this walks the remaining actions from
/// `current_step`, chooses a default [`RecoveryPolicy`] for each action, and
/// calls `handler`. Successful actions advance the journal. The first failed
/// action stops recovery for that entry. Updated journal state is saved before
/// returning.
pub fn recover_incomplete_operations<F>(handler: F) -> FontResult<Vec<ActionRecoveryResult>>
where
    F: Fn(&JournalAction, RecoveryPolicy) -> FontResult<bool>,
{
    let mut journal = load_journal()?;
    let mut results = Vec::new();

    let incomplete_ids: Vec<Uuid> = journal.incomplete_entries().iter().map(|e| e.id).collect();

    for entry_id in incomplete_ids {
        // Get entry details (we need to clone because we'll modify journal later)
        let (remaining, current_step) = {
            let entry = journal.find_entry(entry_id).unwrap();
            (entry.remaining_actions().to_vec(), entry.current_step)
        };

        for (i, action) in remaining.iter().enumerate() {
            let policy = determine_recovery_policy(action);
            let success = handler(action, policy)?;

            results.push(ActionRecoveryResult {
                action: action.clone(),
                policy,
                success,
                message: None,
            });

            if success {
                // Update step
                journal.mark_step(entry_id, current_step + i + 1)?;
            } else {
                // Stop processing this entry on failure
                break;
            }
        }

        // Check if all actions completed
        if let Some(entry) = journal.find_entry(entry_id) {
            if entry.current_step >= entry.actions.len() {
                journal.mark_completed(entry_id)?;
            }
        }
    }

    // Save updated journal
    save_journal(&journal)?;

    Ok(results)
}

/// Choose the built-in recovery policy for one action.
///
/// The current strategy is conservative: continue missing file operations and
/// registrations, skip cache clears, and skip steps that are already satisfied.
fn determine_recovery_policy(action: &JournalAction) -> RecoveryPolicy {
    match action {
        // File operations: roll forward (complete if partially done)
        JournalAction::CopyFile { to, .. } => {
            if to.exists() {
                RecoveryPolicy::Skip // Already done
            } else {
                RecoveryPolicy::RollForward
            }
        }
        JournalAction::DeleteFile { path } => {
            if path.exists() {
                RecoveryPolicy::RollForward
            } else {
                RecoveryPolicy::Skip // Already deleted
            }
        }
        // Registration: roll forward
        JournalAction::RegisterFont { .. } => RecoveryPolicy::RollForward,
        JournalAction::UnregisterFont { .. } => RecoveryPolicy::RollForward,
        // Cache clearing: skip (idempotent, not critical)
        JournalAction::ClearCache { .. } => RecoveryPolicy::Skip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_journal() -> (TempDir, Journal) {
        let temp = TempDir::new().unwrap();
        std::env::set_var("FONTLIFT_JOURNAL_PATH", temp.path().join("journal.json"));
        (temp, Journal::new())
    }

    #[test]
    fn test_journal_entry_creation() {
        let actions = vec![
            JournalAction::CopyFile {
                from: PathBuf::from("/src/font.ttf"),
                to: PathBuf::from("/dst/font.ttf"),
            },
            JournalAction::RegisterFont {
                path: PathBuf::from("/dst/font.ttf"),
                scope: FontScope::User,
            },
        ];

        let entry = JournalEntry::new(actions.clone(), Some("Install font".to_string()));

        assert!(!entry.completed);
        assert_eq!(entry.current_step, 0);
        assert_eq!(entry.actions.len(), 2);
        assert!(entry.is_incomplete());
    }

    #[test]
    fn test_journal_operations() {
        let (_temp, mut journal) = setup_test_journal();

        let actions = vec![JournalAction::DeleteFile {
            path: PathBuf::from("/test.ttf"),
        }];

        let id = journal.record_operation(actions, None);
        assert_eq!(journal.entries.len(), 1);

        journal.mark_step(id, 1).unwrap();
        assert_eq!(journal.find_entry(id).unwrap().current_step, 1);

        journal.mark_completed(id).unwrap();
        assert!(journal.find_entry(id).unwrap().completed);
        assert!(journal.incomplete_entries().is_empty());
    }

    #[test]
    fn test_journal_persistence() {
        // Use a unique temp path and write/read directly instead of relying on env var
        let temp = TempDir::new().unwrap();
        let journal_file = temp.path().join("journal.json");

        let mut journal = Journal::new();
        let actions = vec![JournalAction::ClearCache {
            scope: FontScope::User,
        }];
        journal.record_operation(actions, Some("Cleanup".to_string()));

        // Write directly to temp file
        let content = serde_json::to_string_pretty(&journal).unwrap();
        fs::write(&journal_file, &content).unwrap();

        // Read back
        let read_content = fs::read_to_string(&journal_file).unwrap();
        let loaded: Journal = serde_json::from_str(&read_content).unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].description, Some("Cleanup".to_string()));
    }

    #[test]
    fn test_action_descriptions() {
        let copy = JournalAction::CopyFile {
            from: PathBuf::from("/a"),
            to: PathBuf::from("/b"),
        };
        assert!(copy.description().contains("/a"));
        assert!(copy.description().contains("/b"));

        let register = JournalAction::RegisterFont {
            path: PathBuf::from("/font.ttf"),
            scope: FontScope::System,
        };
        assert!(register.description().contains("font.ttf"));
        assert!(register.description().contains("System"));
    }

    #[test]
    fn test_cleanup_old_entries() {
        let mut journal = Journal::new();

        // Add a completed entry
        let id = journal.record_operation(vec![], None);
        journal.mark_completed(id).unwrap();

        // Add an incomplete entry
        journal.record_operation(
            vec![JournalAction::ClearCache {
                scope: FontScope::User,
            }],
            None,
        );

        assert_eq!(journal.entries.len(), 2);

        // Cleanup with 0 age should remove completed, keep incomplete
        journal.cleanup_old_entries(0);

        assert_eq!(journal.entries.len(), 1);
        assert!(journal.entries[0].is_incomplete());
    }

    #[test]
    fn test_recovery_policy_determination() {
        let copy_missing = JournalAction::CopyFile {
            from: PathBuf::from("/nonexistent"),
            to: PathBuf::from("/also_nonexistent"),
        };
        assert_eq!(
            determine_recovery_policy(&copy_missing),
            RecoveryPolicy::RollForward
        );

        let cache = JournalAction::ClearCache {
            scope: FontScope::User,
        };
        assert_eq!(determine_recovery_policy(&cache), RecoveryPolicy::Skip);
    }
}
