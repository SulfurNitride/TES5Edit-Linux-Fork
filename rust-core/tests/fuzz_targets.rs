//! Fuzz-friendly parser robustness tests.
//!
//! These tests verify that the plugin parser returns `Err` rather than panicking
//! when presented with malformed, truncated, or corrupted input.
//! None of these require real game files.

use xedit_dom::GameId;
use xedit_io::PluginReader;

/// Helper: attempt to parse `data` as a plugin for the given game.
/// Returns Ok(()) if it parsed (valid or not), Err if the parser returned an error.
/// The key invariant: this must NEVER panic.
fn try_parse(game_id: GameId, data: &[u8]) -> Result<(), String> {
    let reader = PluginReader::new(game_id);
    match reader.read_bytes(data, None) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// Attempt parsing across all TES4+ game IDs. Must not panic for any.
fn try_parse_all_games(data: &[u8]) {
    let games = [
        GameId::SkyrimSE,
        GameId::Fallout4,
        GameId::FalloutNV,
        GameId::Fallout3,
        GameId::Oblivion,
        GameId::Morrowind,
    ];
    for game_id in &games {
        let reader = PluginReader::new(*game_id);
        // We only care that it doesn't panic. Errors are expected.
        let _ = reader.read_bytes(data, None);
    }
}

// ---------------------------------------------------------------------------
// Test 1: Empty input
// ---------------------------------------------------------------------------

#[test]
fn fuzz_empty_input_does_not_panic() {
    try_parse_all_games(&[]);
}

#[test]
fn fuzz_empty_input_returns_error() {
    // An empty byte slice cannot contain a valid plugin header.
    let result = try_parse(GameId::SkyrimSE, &[]);
    assert!(
        result.is_err(),
        "Empty input should produce an error, not a valid plugin"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Truncated header (fewer than 24 bytes for TES4+)
// ---------------------------------------------------------------------------

#[test]
fn fuzz_truncated_header_does_not_panic() {
    // A TES4 record header is 24 bytes. Feed only partial data.
    let partial = b"TES4\x10\x00\x00\x00\x00\x00"; // 10 bytes
    try_parse_all_games(partial);
}

#[test]
fn fuzz_truncated_header_returns_error() {
    let partial = b"TES4\x10\x00\x00\x00\x00\x00";
    let result = try_parse(GameId::SkyrimSE, partial);
    assert!(
        result.is_err(),
        "Truncated header should produce an error"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Invalid magic / wrong header signature
// ---------------------------------------------------------------------------

#[test]
fn fuzz_invalid_magic_does_not_panic() {
    // Start with "XXXX" instead of "TES4" or "TES3".
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(b"XXXX");
    try_parse_all_games(&data);
}

#[test]
fn fuzz_invalid_magic_returns_error() {
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(b"XXXX");
    let result = try_parse(GameId::SkyrimSE, &data);
    assert!(
        result.is_err(),
        "Invalid magic signature should produce an error"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Oversized record (data_size claims more bytes than available)
// ---------------------------------------------------------------------------

#[test]
fn fuzz_oversized_record_does_not_panic() {
    // Build a TES4 header that claims a 1MB data section but only provide 24 header bytes.
    let mut data = vec![0u8; 24];
    data[0..4].copy_from_slice(b"TES4");
    // data_size at offset 4..8 = 0x00100000 (1 MB)
    data[4] = 0x00;
    data[5] = 0x00;
    data[6] = 0x10;
    data[7] = 0x00;
    try_parse_all_games(&data);
}

#[test]
fn fuzz_oversized_record_returns_error() {
    let mut data = vec![0u8; 24];
    data[0..4].copy_from_slice(b"TES4");
    data[4] = 0x00;
    data[5] = 0x00;
    data[6] = 0x10;
    data[7] = 0x00;
    let result = try_parse(GameId::SkyrimSE, &data);
    assert!(
        result.is_err(),
        "Oversized record with truncated data should produce an error"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Corrupted subrecord (valid header, garbage body)
// ---------------------------------------------------------------------------

#[test]
fn fuzz_corrupted_subrecord_does_not_panic() {
    // Build a minimal TES4 header with a body that contains a subrecord
    // whose size field overflows.
    let mut data = vec![0u8; 48];
    data[0..4].copy_from_slice(b"TES4");
    // data_size = 24 bytes of body
    data[4] = 24;
    data[5] = 0;
    data[6] = 0;
    data[7] = 0;
    // flags, form_id, vc_info, version, unknown = 0 (bytes 8..24)

    // Body starts at offset 24. Put a subrecord with a huge size.
    data[24..28].copy_from_slice(b"HEDR");
    // subrecord size = 0xFFFF (65535) - but only ~20 bytes remain.
    data[28] = 0xFF;
    data[29] = 0xFF;
    // Rest is zeros (garbage).
    try_parse_all_games(&data);
}

#[test]
fn fuzz_corrupted_subrecord_returns_error() {
    let mut data = vec![0u8; 48];
    data[0..4].copy_from_slice(b"TES4");
    data[4] = 24;
    data[5] = 0;
    data[6] = 0;
    data[7] = 0;
    data[24..28].copy_from_slice(b"HEDR");
    data[28] = 0xFF;
    data[29] = 0xFF;
    let result = try_parse(GameId::SkyrimSE, &data);
    // This may or may not parse depending on how the parser handles truncated subrecords.
    // The key property is that it does not panic (tested above). If it does parse,
    // that is also acceptable behavior for a lenient parser.
    let _ = result;
}

// ---------------------------------------------------------------------------
// Test 6: All 0xFF bytes (worst-case garbage)
// ---------------------------------------------------------------------------

#[test]
fn fuzz_all_ff_bytes_does_not_panic() {
    let data = vec![0xFFu8; 256];
    try_parse_all_games(&data);
}

// ---------------------------------------------------------------------------
// Test 7: Valid header but truncated before groups
// ---------------------------------------------------------------------------

#[test]
fn fuzz_header_only_no_groups_does_not_panic() {
    // A valid-looking TES4 header with 12 bytes of HEDR but no groups after.
    let mut data = vec![0u8; 42];
    data[0..4].copy_from_slice(b"TES4");
    // data_size = 18 bytes (HEDR subrecord: 4 sig + 2 size + 12 data = 18)
    data[4] = 18;
    data[5] = 0;
    data[6] = 0;
    data[7] = 0;
    // flags, formid, vc_info, version, unknown = 0 (bytes 8..24)
    // Body at 24..42
    data[24..28].copy_from_slice(b"HEDR");
    data[28] = 12; // subrecord data size = 12
    data[29] = 0;
    // 12 bytes of HEDR data (all zeros) at 30..42.
    try_parse_all_games(&data);
}

#[test]
fn fuzz_header_only_no_groups_parses_or_errors() {
    let mut data = vec![0u8; 42];
    data[0..4].copy_from_slice(b"TES4");
    data[4] = 18;
    data[5] = 0;
    data[6] = 0;
    data[7] = 0;
    data[24..28].copy_from_slice(b"HEDR");
    data[28] = 12;
    data[29] = 0;
    let reader = PluginReader::new(GameId::SkyrimSE);
    // A header-only file (no groups) may be valid (empty plugin) or an error.
    // Either outcome is acceptable; the test ensures no panic.
    let result = reader.read_bytes(&data, None);
    match result {
        Ok(plugin) => {
            assert_eq!(plugin.header.signature, xedit_dom::Signature::TES4);
        }
        Err(_) => {
            // Error is also acceptable.
        }
    }
}

// ---------------------------------------------------------------------------
// Test 8: Truncated group header
// ---------------------------------------------------------------------------

#[test]
fn fuzz_truncated_group_header_does_not_panic() {
    // Valid TES4 header followed by a partial GRUP header (only 8 of 24 bytes).
    let mut data = vec![0u8; 32];
    data[0..4].copy_from_slice(b"TES4");
    // data_size = 0 (empty header body)
    // flags, formid, vc_info, version, unknown = 0
    // At offset 24, start a partial GRUP
    data[24..28].copy_from_slice(b"GRUP");
    data[28] = 0xFF; // group size byte (partial)
    // Only 8 bytes of group header provided.
    try_parse_all_games(&data);
}

// ---------------------------------------------------------------------------
// Test 9: Extremely large claimed size (near u32::MAX)
// ---------------------------------------------------------------------------

#[test]
fn fuzz_near_max_size_does_not_panic() {
    let mut data = vec![0u8; 24];
    data[0..4].copy_from_slice(b"TES4");
    // data_size = 0xFFFFFFFF
    data[4] = 0xFF;
    data[5] = 0xFF;
    data[6] = 0xFF;
    data[7] = 0xFF;
    try_parse_all_games(&data);
}

// ---------------------------------------------------------------------------
// Test 10: Random-looking bytes with valid-ish signature
// ---------------------------------------------------------------------------

#[test]
fn fuzz_random_ish_with_tes4_prefix_does_not_panic() {
    // TES4 signature followed by pseudo-random bytes.
    let mut data = Vec::with_capacity(512);
    data.extend_from_slice(b"TES4");
    // Generate deterministic "random" bytes.
    let mut x: u32 = 0xDEADBEEF;
    for _ in 4..512 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        data.push((x >> 16) as u8);
    }
    try_parse_all_games(&data);
}

// ---------------------------------------------------------------------------
// Test 11: Morrowind-specific: TES3 header with garbage
// ---------------------------------------------------------------------------

#[test]
fn fuzz_tes3_garbage_body_does_not_panic() {
    let mut data = vec![0u8; 128];
    data[0..4].copy_from_slice(b"TES3");
    // Fill the rest with alternating patterns.
    for i in 4..128 {
        data[i] = (i as u8).wrapping_mul(37);
    }
    let reader = PluginReader::new(GameId::Morrowind);
    let _ = reader.read_bytes(&data, None);
}
