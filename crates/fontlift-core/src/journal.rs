//! Transactional operation journal for crash-safe font operations
//!
//! This module provides a small operation journal that tracks multi-step
//! operations (install, remove, cleanup) so that interrupted operations
//! can be detected and repaired on the next run.

use crate::{FontError, FontResult, FontScope};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// Actions that can be recorded in the journal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JournalAction {
    /// Copy a file from source to destination
    CopyFile { from: PathBuf, to: PathBuf },
    /// Register a font with the OS
    RegisterFont { path: PathBuf, scope: FontScope },
    /// Unregister a font from the OS
    UnregisterFont { path: PathBuf, scope: FontScope },
    /// Delete a file
    DeleteFile { path: PathBuf },
    /// Clear font caches
    ClearCache { scope: FontScope },
}

impl JournalAction {
    /// Human-readable description of the action
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

/// A journal entry representing a multi-step operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique identifier for this operation
    pub id: Uuid,
    /// When the operation started
    #[serde(with = "systemtime_serde")]
    pub started_at: SystemTime,
    /// Whether the operation completed successfully
    pub completed: bool,
    /// The actions to perform (in order)
    pub actions: Vec<JournalAction>,
    /// Index of the current step (0-based)
    pub current_step: usize,
    /// Optional description of the operation
    pub description: Option<String>,
}

impl JournalEntry {
    /// Create a new journal entry
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

    /// Check if this entry is incomplete (started but not finished)
    pub fn is_incomplete(&self) -> bool {
        !self.completed && !self.actions.is_empty()
    }

    /// Get the current action (if any)
    pub fn current_action(&self) -> Option<&JournalAction> {
        self.actions.get(self.current_step)
    }

    /// Get remaining actions (from current step onwards)
    pub fn remaining_actions(&self) -> &[JournalAction] {
        if self.current_step < self.actions.len() {
            &self.actions[self.current_step..]
        } else {
            &[]
        }
    }
}

/// SystemTime serde helpers
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

/// The journal containing all entries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    pub entries: Vec<JournalEntry>,
}

impl Journal {
    /// Create an empty journal
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an entry and return its ID
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

    /// Find an entry by ID
    pub fn find_entry(&self, id: Uuid) -> Option<&JournalEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Find an entry by ID (mutable)
    pub fn find_entry_mut(&mut self, id: Uuid) -> Option<&mut JournalEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Mark a step as completed
    pub fn mark_step(&mut self, id: Uuid, step: usize) -> FontResult<()> {
        let entry = self
            .find_entry_mut(id)
            .ok_or_else(|| FontError::InvalidFormat(format!("Journal entry not found: {id}")))?;
        entry.current_step = step;
        Ok(())
    }

    /// Mark an operation as completed
    pub fn mark_completed(&mut self, id: Uuid) -> FontResult<()> {
        let entry = self
            .find_entry_mut(id)
            .ok_or_else(|| FontError::InvalidFormat(format!("Journal entry not found: {id}")))?;
        entry.completed = true;
        Ok(())
    }

    /// Get all incomplete entries
    pub fn incomplete_entries(&self) -> Vec<&JournalEntry> {
        self.entries.iter().filter(|e| e.is_incomplete()).collect()
    }

    /// Remove completed entries older than the specified duration
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

/// Get the platform-specific journal file path
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

/// Load the journal from disk
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

/// Save the journal to disk (atomic write)
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

/// Policy for recovering incomplete operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPolicy {
    /// Roll forward: try to complete the operation
    RollForward,
    /// Roll back: undo what was done
    RollBack,
    /// Skip: mark as completed without action
    Skip,
}

/// Result of attempting to recover a single action
#[derive(Debug)]
pub struct ActionRecoveryResult {
    pub action: JournalAction,
    pub policy: RecoveryPolicy,
    pub success: bool,
    pub message: Option<String>,
}

/// Recover incomplete operations
///
/// This function iterates through incomplete journal entries and attempts
/// to either complete them (roll forward) or undo them (roll back).
///
/// # Arguments
/// * `handler` - Callback to execute recovery actions
///
/// # Returns
/// Results of recovery attempts
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

/// Determine the default recovery policy for an action
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
