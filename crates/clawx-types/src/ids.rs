use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Create a new random ID.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Return the inner UUID.
            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(AgentId, "Unique identifier for an agent.");
define_id!(TaskId, "Unique identifier for a task.");
define_id!(MemoryId, "Unique identifier for a memory entry.");
define_id!(SkillId, "Unique identifier for a skill.");
define_id!(ChannelId, "Unique identifier for a channel.");
define_id!(EventId, "Unique identifier for an event.");
define_id!(SnapshotId, "Unique identifier for a snapshot.");
define_id!(ConversationId, "Unique identifier for a conversation.");
define_id!(ChunkId, "Unique identifier for a knowledge-base chunk.");
