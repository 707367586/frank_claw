//! File system watcher using FSEvents (macOS) via the `notify` crate.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// The kind of file system event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// A file system event.
#[derive(Debug, Clone)]
pub struct FsEvent {
    pub kind: FsEventKind,
    pub paths: Vec<PathBuf>,
}

/// File system watcher that monitors directories for changes.
pub struct FsWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::UnboundedReceiver<FsEvent>,
}

impl FsWatcher {
    /// Create a new watcher monitoring the given paths.
    pub fn new(paths: &[&Path]) -> Result<Self, String> {
        let (tx, rx) = mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    let kind = match event.kind {
                        EventKind::Create(_) => Some(FsEventKind::Created),
                        EventKind::Modify(_) => Some(FsEventKind::Modified),
                        EventKind::Remove(_) => Some(FsEventKind::Deleted),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        let _ = tx.send(FsEvent {
                            kind,
                            paths: event.paths,
                        });
                    }
                }
                Err(e) => {
                    warn!("fs watcher error: {}", e);
                }
            }
        })
        .map_err(|e| format!("failed to create watcher: {}", e))?;

        for path in paths {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| format!("failed to watch {}: {}", path.display(), e))?;
            info!(path = %path.display(), "watching directory");
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }

    /// Receive the next file system event.
    /// Returns `None` if the watcher is dropped.
    pub async fn next_event(&mut self) -> Option<FsEvent> {
        self.receiver.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_watcher_detects_file_creation() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        let mut watcher = FsWatcher::new(&[dir_path]).unwrap();

        // Create a file in the watched directory.
        let file_path = dir_path.join("new_file.txt");
        tokio::fs::write(&file_path, b"hello").await.unwrap();

        let event = timeout(Duration::from_secs(5), watcher.next_event())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert!(
            event.kind == FsEventKind::Created || event.kind == FsEventKind::Modified,
            "expected Created or Modified, got {:?}",
            event.kind
        );
    }

    #[tokio::test]
    async fn test_watcher_detects_file_modification() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // Create a file before watching.
        let file_path = dir_path.join("existing.txt");
        std::fs::write(&file_path, b"initial").unwrap();

        let mut watcher = FsWatcher::new(&[dir_path]).unwrap();

        // Modify the file.
        tokio::fs::write(&file_path, b"modified").await.unwrap();

        let event = timeout(Duration::from_secs(5), watcher.next_event())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert!(
            event.kind == FsEventKind::Modified || event.kind == FsEventKind::Created,
            "expected Modified or Created, got {:?}",
            event.kind
        );
    }

    #[tokio::test]
    async fn test_watcher_detects_file_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // Create a file before watching.
        let file_path = dir_path.join("to_delete.txt");
        std::fs::write(&file_path, b"delete me").unwrap();

        let mut watcher = FsWatcher::new(&[dir_path]).unwrap();

        // Small delay to let the watcher settle after initial setup.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Delete the file.
        tokio::fs::remove_file(&file_path).await.unwrap();

        // Drain events until we see a Deleted (or timeout).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut saw_deleted = false;
        while tokio::time::Instant::now() < deadline {
            match timeout(Duration::from_secs(3), watcher.next_event()).await {
                Ok(Some(event)) if event.kind == FsEventKind::Deleted => {
                    saw_deleted = true;
                    break;
                }
                Ok(Some(_)) => continue, // skip non-delete events
                _ => break,
            }
        }
        assert!(saw_deleted, "expected a Deleted event within timeout");
    }
}
