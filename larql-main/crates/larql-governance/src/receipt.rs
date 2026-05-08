use crate::patch_pipeline::PatchEnvelope;
use crate::policy::PolicyDecision;
use crate::struct_minter::{CargoCheckStatus, IntegrationMode, MintChecks};
use larql_core::capsule::{make_capsule, make_capture, Capsule, CapsuleError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MACHINE_PRODUCER: &str = "larql-governance";
pub const MINT_RECEIPT_CAPTURE_KIND: &str = "larql.governance.struct_mint.receipt.v1";
pub const MINT_RECEIPT_CAPSULE_KIND: &str = "larql.governance.struct_mint.receipt_capsule.v1";
pub const POLICY_RECEIPT_CAPTURE_KIND: &str = "larql.governance.policy.receipt.v1";
pub const POLICY_RECEIPT_CAPSULE_KIND: &str = "larql.governance.policy.receipt_capsule.v1";
pub const PATCH_ENVELOPE_CAPTURE_KIND: &str = "larql.governance.patch_envelope.v1";
pub const PATCH_ENVELOPE_CAPSULE_KIND: &str = "larql.governance.patch_envelope_capsule.v1";
pub const DENIAL_CAPTURE_KIND: &str = "larql.governance.denial.v1";
pub const DENIAL_CAPSULE_KIND: &str = "larql.governance.denial_capsule.v1";

#[derive(Debug, Error)]
pub enum ReceiptError {
    #[error(transparent)]
    Capsule(#[from] CapsuleError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MintReceipt {
    pub schema_version: String,
    pub event_type: String,
    pub machine: String,
    pub spec_sha256: String,
    pub type_need_hash: String,
    pub evidence_hashes: Vec<String>,
    pub generated_item_sha256: String,
    pub target_path: String,
    pub integration_mode: IntegrationMode,
    pub checks: MintChecks,
    pub cargo_check: CargoCheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyReceipt {
    pub schema_version: String,
    pub event_type: String,
    pub profile: String,
    pub spec_sha256: Option<String>,
    pub decision: PolicyDecision,
}

pub type PatchEnvelopeReceipt = PatchEnvelope;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineDenial {
    pub schema_version: String,
    pub event_type: String,
    pub machine: String,
    pub rule: String,
    pub denied_field: String,
    pub required_ceremony: String,
    pub evidence_hashes: Vec<String>,
}

pub fn make_mint_receipt_capsule(
    receipt: MintReceipt,
) -> Result<Capsule<MintReceipt>, ReceiptError> {
    let capture = make_capture(MINT_RECEIPT_CAPTURE_KIND, receipt)?;
    Ok(make_capsule(
        MINT_RECEIPT_CAPSULE_KIND,
        capture,
        MACHINE_PRODUCER,
    )?)
}

pub fn make_policy_receipt_capsule(
    receipt: PolicyReceipt,
) -> Result<Capsule<PolicyReceipt>, ReceiptError> {
    let capture = make_capture(POLICY_RECEIPT_CAPTURE_KIND, receipt)?;
    Ok(make_capsule(
        POLICY_RECEIPT_CAPSULE_KIND,
        capture,
        MACHINE_PRODUCER,
    )?)
}

pub fn make_patch_envelope_capsule(
    receipt: PatchEnvelopeReceipt,
) -> Result<Capsule<PatchEnvelopeReceipt>, ReceiptError> {
    let capture = make_capture(PATCH_ENVELOPE_CAPTURE_KIND, receipt)?;
    Ok(make_capsule(
        PATCH_ENVELOPE_CAPSULE_KIND,
        capture,
        MACHINE_PRODUCER,
    )?)
}

pub fn make_denial_capsule(denial: MachineDenial) -> Result<Capsule<MachineDenial>, ReceiptError> {
    let capture = make_capture(DENIAL_CAPTURE_KIND, denial)?;
    Ok(make_capsule(
        DENIAL_CAPSULE_KIND,
        capture,
        MACHINE_PRODUCER,
    )?)
}
