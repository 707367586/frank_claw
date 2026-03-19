use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

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

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
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
define_id!(SnapshotId, "Unique identifier for a vault snapshot.");
define_id!(ConversationId, "Unique identifier for a conversation.");
define_id!(MessageId, "Unique identifier for a message.");
define_id!(ChunkId, "Unique identifier for a knowledge-base chunk.");
define_id!(DocumentId, "Unique identifier for a knowledge-base document.");
define_id!(KnowledgeSourceId, "Unique identifier for a knowledge source.");
define_id!(ProviderId, "Unique identifier for an LLM provider.");
define_id!(SessionId, "Unique identifier for a memory session.");
define_id!(AuditEntryId, "Unique identifier for an audit log entry.");
