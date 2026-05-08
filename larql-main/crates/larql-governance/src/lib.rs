pub mod binding;
pub mod ceremony;
pub mod hash;
pub mod patch_pipeline;
pub mod policy;
pub mod receipt;
pub mod struct_minter;

pub use binding::{derive_struct_bindings, RustStructBinding};
pub use ceremony::{
    CapabilityScope, CeremonyEvent, CeremonyReceipt, MachineCapability, MachineChannel,
};
pub use hash::{hash_bytes, hash_json, hash_text, HashRef};
pub use patch_pipeline::{
    plan_governed_patch, Confidence, GateAnswer, ImpactLevel, IntentKind, PatchAnswerToken,
    PatchEnvelope, PatchEnvelopeStatus, PatchIntentSpec, PatchPipelineError, PatchStage,
    PatchStageKind, RepoFactSummary, StageStatus,
};
pub use policy::{
    validate_machine_profile, MachinePolicyEngine, MachineRuleProfile, PolicyDecision, PolicyError,
    StructClassifications,
};
pub use receipt::{
    make_denial_capsule, make_mint_receipt_capsule, make_patch_envelope_capsule,
    make_policy_receipt_capsule, MachineDenial, MintReceipt, PatchEnvelopeReceipt, PolicyReceipt,
    ReceiptError, DENIAL_CAPSULE_KIND, DENIAL_CAPTURE_KIND, MINT_RECEIPT_CAPSULE_KIND,
    MINT_RECEIPT_CAPTURE_KIND, PATCH_ENVELOPE_CAPSULE_KIND, PATCH_ENVELOPE_CAPTURE_KIND,
    POLICY_RECEIPT_CAPSULE_KIND, POLICY_RECEIPT_CAPTURE_KIND,
};
pub use struct_minter::{
    mint_struct, mint_struct_with_profile, verify_struct_spec, verify_struct_spec_with_profile,
    CargoCheckStatus, FieldSpec, GovernanceSpec, IntegrationMode, LayoutSpec, MintChecks,
    MintOutcome, StructItemSpec, StructSpec,
};
