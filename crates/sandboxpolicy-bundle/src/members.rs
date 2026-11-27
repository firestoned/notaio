// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The members bundle schema v4 adds (ADR-0026 decision 1), and the bundle's own entry types,
//! which carry an entry's origin in a composite (`from`, ADR-0023 decision 7) without putting it in
//! the CRD's types (ADR-0026 decision 2).

use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use sandboxpolicy_types::{AudienceGrant, EgressRule, ToolRef};
use serde::{Deserialize, Serialize};

/// Who may claim a sandbox under the policy (ADR-0024): identity provider groups, matched
/// exactly, sorted by byte order. `[]` means nobody.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subjects {
    pub groups: Vec<String>,
}

/// One policy a composite includes (ADR-0023 decision 7), as its `status.includes` records it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncludeRef {
    pub name: String,
    pub uid: String,
    /// `sha256:` of the RFC 8785 form of what the include contributes.
    pub content_digest: String,
}

/// The sandbox's size for K2.4 (ADR-0019). Not filled until ADR-0019 is built.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub vcpus: u32,
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u32,
    #[serde(rename = "diskGiB")]
    pub disk_gib: u32,
}

/// Where the policy, its ceiling and its grants came from, as the delivery pipeline claimed it
/// (ADR-0016). Copied, never verified. Not filled until ADR-0016 is built.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling: Option<String>,
    /// By grant name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub grants: BTreeMap<String, String>,
}

/// A bundle entry: the shared type, plus the include it came from when an include supplied it.
/// Absent `from` serialises exactly as the shared type alone, so a v3 entry is unchanged.
macro_rules! bundle_entry {
    ($(#[$doc:meta])* $name:ident, $field:ident, $inner:ty) => {
        $(#[$doc])*
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            #[serde(flatten)]
            pub $field: $inner,
            /// The include that supplied this entry; absent for the policy's own (ADR-0023).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub from: Option<String>,
        }

        impl From<$inner> for $name {
            fn from($field: $inner) -> Self {
                $name { $field, from: None }
            }
        }

        impl Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &$inner {
                &self.$field
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut $inner {
                &mut self.$field
            }
        }
    };
}

bundle_entry!(
    /// An audience grant in a bundle.
    BundleAudience,
    grant,
    AudienceGrant
);
bundle_entry!(
    /// A direct egress rule in a bundle.
    BundleEgress,
    rule,
    EgressRule
);
bundle_entry!(
    /// A pinned tool in a bundle.
    BundleTool,
    tool,
    ToolRef
);
