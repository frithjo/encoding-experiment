use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoidBoardProjection {
    pub schema_version: String,
    pub generated_by: String,
    pub location_groups: Vec<LocationGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocationGroup {
    pub location: String,
    pub features: Vec<FeatureGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureGroup {
    pub feature: String,
    pub factoids: Vec<FactoidNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoidNode {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub rust_paths: Vec<RustPathNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustPathNode {
    pub path: String,
    pub symbols: Vec<RustSymbolNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustSymbolNode {
    pub name: String,
    pub kind: RustSymbolKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RustSymbolKind {
    Struct,
    Enum,
    Trait,
    Function,
    Module,
    TypeAlias,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactRecord {
    pub id: String,
    pub location: String,
    pub feature: String,
    pub title: String,
    pub summary: String,
    pub rust_paths: Vec<RustPathRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustPathRecord {
    pub path: String,
    pub symbols: Vec<RustSymbolNode>,
}

pub fn project_factoid_board(records: &[FactRecord]) -> FactoidBoardProjection {
    let mut locations: BTreeMap<String, BTreeMap<String, Vec<&FactRecord>>> = BTreeMap::new();

    for record in records {
        let feature_map = locations.entry(record.location.clone()).or_default();
        feature_map
            .entry(record.feature.clone())
            .or_default()
            .push(record);
    }

    let mut location_groups = Vec::new();
    for (location, features) in locations {
        let mut feature_groups = Vec::new();
        for (feature, facts) in features {
            let mut factoids = Vec::new();
            for fact in facts {
                factoids.push(FactoidNode {
                    id: fact.id.clone(),
                    title: fact.title.clone(),
                    summary: fact.summary.clone(),
                    rust_paths: fact
                        .rust_paths
                        .iter()
                        .map(|path| RustPathNode {
                            path: path.path.clone(),
                            symbols: path.symbols.clone(),
                        })
                        .collect(),
                });
            }
            feature_groups.push(FeatureGroup { feature, factoids });
        }
        location_groups.push(LocationGroup {
            location,
            features: feature_groups,
        });
    }

    FactoidBoardProjection {
        schema_version: "larql.factboard.projection.v1".to_string(),
        generated_by: "larql-factboard".to_string(),
        location_groups,
    }
}

pub fn sample_fact_records() -> Vec<FactRecord> {
    vec![
        FactRecord {
            id: "fact-001".to_string(),
            location: "governance".to_string(),
            feature: "policy-engine".to_string(),
            title: "Policy evaluation captures matched rules".to_string(),
            summary: "Runtime policy decisions carry matched rule IDs and findings.".to_string(),
            rust_paths: vec![
                RustPathRecord {
                    path: "crates/larql-governance/src/rules_engine.rs".to_string(),
                    symbols: vec![
                        RustSymbolNode {
                            name: "PolicyEngine".to_string(),
                            kind: RustSymbolKind::Struct,
                        },
                        RustSymbolNode {
                            name: "evaluate".to_string(),
                            kind: RustSymbolKind::Function,
                        },
                        RustSymbolNode {
                            name: "PolicyEngineDecision".to_string(),
                            kind: RustSymbolKind::Struct,
                        },
                    ],
                },
                RustPathRecord {
                    path: "crates/larql-cli/src/commands/machine_cmd.rs".to_string(),
                    symbols: vec![RustSymbolNode {
                        name: "run_rules".to_string(),
                        kind: RustSymbolKind::Function,
                    }],
                },
            ],
        },
        FactRecord {
            id: "fact-002".to_string(),
            location: "ui".to_string(),
            feature: "projection".to_string(),
            title: "UI consumes Rust-owned board projection".to_string(),
            summary: "Factoid board hierarchy is derived in Rust and projected to WASM view."
                .to_string(),
            rust_paths: vec![RustPathRecord {
                path: "crates/larql-factboard/src/lib.rs".to_string(),
                symbols: vec![
                    RustSymbolNode {
                        name: "FactoidBoardProjection".to_string(),
                        kind: RustSymbolKind::Struct,
                    },
                    RustSymbolNode {
                        name: "project_factoid_board".to_string(),
                        kind: RustSymbolKind::Function,
                    },
                ],
            }],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_records_by_location_then_feature() {
        let projection = project_factoid_board(&sample_fact_records());
        assert_eq!(projection.schema_version, "larql.factboard.projection.v1");
        assert_eq!(projection.location_groups.len(), 2);
        assert_eq!(projection.location_groups[0].location, "governance");
        assert_eq!(
            projection.location_groups[0].features[0].feature,
            "policy-engine"
        );
    }
}
