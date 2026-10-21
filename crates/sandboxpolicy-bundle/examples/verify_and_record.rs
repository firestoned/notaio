//! The consumer loop (ADR-0008): verify a bundle, check it against the persisted version floor,
//! record the new floor, and only then act on it. mediatore, mediatore-guest and the gateway do
//! exactly this, with their own way of obtaining the envelope and the trusted key.
//!
//! ```sh
//! cargo run -p sandboxpolicy-bundle --example verify_and_record -- \
//!     bundle.dsse.json <key-id> <public-key-base64> /var/lib/consumer/floor.json
//! ```
//!
//! `bundle.dsse.json` is the `bundle.dsse.json` key of a published `<policy>-bundle` ConfigMap.
//! Exit codes: 0 accepted, 1 refused, 2 bad arguments or unreadable input.

use std::path::Path;
use std::process::ExitCode;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use sandboxpolicy_bundle::floor::{accept, FileFloor};
use sandboxpolicy_bundle::{Envelope, TrustedKeys};

const EXIT_REFUSED: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [envelope, key_id, public_key, floor] = args.as_slice() else {
        eprintln!(
            "usage: verify_and_record <envelope.json> <key-id> <public-key-base64> <floor.json>"
        );
        return ExitCode::from(EXIT_USAGE);
    };
    match run(envelope, key_id, public_key, floor) {
        Ok(Ok(summary)) => {
            println!("accepted: {summary}");
            ExitCode::SUCCESS
        }
        Ok(Err(refusal)) => {
            // Keep the last good bundle and carry on: never act on this one.
            eprintln!("refused: {refusal}");
            ExitCode::from(EXIT_REFUSED)
        }
        Err(input) => {
            eprintln!("error: {input}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Outer `Err` is bad input; inner `Err` is a refusal by the verifier or the floor.
fn run(
    envelope: &str,
    key_id: &str,
    public_key: &str,
    floor: &str,
) -> Result<Result<String, String>, String> {
    let text = std::fs::read_to_string(envelope).map_err(|e| format!("{envelope}: {e}"))?;
    let env: Envelope = serde_json::from_str(&text).map_err(|e| format!("{envelope}: {e}"))?;
    let raw = STANDARD
        .decode(public_key.trim())
        .map_err(|e| format!("public key: {e}"))?;
    let public: [u8; 32] = raw
        .try_into()
        .map_err(|_| "public key: must be 32 bytes".to_string())?;
    let mut keys = TrustedKeys::new();
    keys.insert(key_id, public).map_err(|e| e.to_string())?;
    let mut floor = FileFloor::open(Path::new(floor)).map_err(|e| e.to_string())?;

    Ok(accept(&env, &keys, Utc::now(), &mut floor)
        .map(|p| {
            format!(
                "{}/{} version {} (uid {}), valid until {}",
                p.content.policy.namespace,
                p.content.policy.name,
                p.meta.version,
                p.content.policy.uid,
                p.meta.not_after.to_rfc3339()
            )
        })
        .map_err(|e| e.to_string()))
}
