//! Shared protocol-decode conformance test.
//!
//! Runs the canonical vectors in `tests/vectors/protocol_decode.json` through the
//! engine's `Reader`. The TypeScript web engine (in the sibling `nowhere` repo)
//! runs the SAME file through its own `Reader` — so if the two decoders ever
//! diverge, one of the suites goes red. Edit the vectors in one place.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use trot_core::drivers::lifespan::Reader;

fn hex_to_bytes(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn field(t: &trot_core::drivers::lifespan::Readout, name: &str) -> i64 {
    match name {
        "steps" => t.steps.map(|v| v as i64).expect("steps unset"),
        "duration_s" => t.duration_s.map(|v| v as i64).expect("duration_s unset"),
        "distance_raw" => t
            .distance_raw
            .map(|v| v as i64)
            .expect("distance_raw unset"),
        "calories" => t.calories.map(|v| v as i64).expect("calories unset"),
        "speed_raw" => t.speed_raw.map(|v| v as i64).expect("speed_raw unset"),
        "status" => t.status.map(|v| v as i64).expect("status unset"),
        other => panic!("unknown expect field: {other}"),
    }
}

#[test]
fn protocol_decode_vectors() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "vectors",
        "protocol_decode.json",
    ]
    .iter()
    .collect();
    let raw = fs::read_to_string(&path).expect("read vectors");
    let doc: Value = serde_json::from_str(&raw).expect("parse vectors");
    let cases = doc["cases"].as_array().expect("cases array");

    let mut ran = 0;
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let opcode = {
            let s = case["opcode"].as_str().expect("opcode");
            u8::from_str_radix(s.trim_start_matches("0x"), 16).expect("opcode hex")
        };
        let frame = hex_to_bytes(case["frame"].as_str().expect("frame"));

        let mut reader = Reader::new();
        let result = reader.feed(opcode, &frame);

        if case.get("error").and_then(|e| e.as_bool()).unwrap_or(false) {
            assert!(
                result.is_err(),
                "case '{name}': expected decode error, got {result:?}"
            );
        } else {
            let telem = result.unwrap_or_else(|e| panic!("case '{name}': unexpected error {e:?}"));
            let want = case["expect"]["value"].as_i64().expect("expect.value");
            let got = field(
                &telem,
                case["expect"]["field"].as_str().expect("expect.field"),
            );
            assert_eq!(got, want, "case '{name}': field mismatch");
        }
        ran += 1;
    }
    assert!(ran >= 15, "expected the full vector set, only ran {ran}");
    eprintln!("protocol_decode_vectors: {ran} shared cases passed");
}
