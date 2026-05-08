use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MachineChannel {
    UserIntent,
    ModelIntent,
    System,
    Machine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityScope {
    pub action: String,
    pub allowed_paths: Vec<String>,
    pub prior_state_hash: String,
    pub expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineCapability {
    pub schema_version: String,
    pub capability_id: String,
    pub actor_hash: String,
    pub ceremony_hash: String,
    pub scope: CapabilityScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CeremonyEvent {
    pub event_type: String,
    pub channel: MachineChannel,
    pub event_hash: String,
    pub prior_event_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CeremonyReceipt {
    pub schema_version: String,
    pub ceremony: String,
    pub status: String,
    pub events: Vec<CeremonyEvent>,
    pub receipt_hash: String,
}
