//! Parser for Delphi wb* definition patterns from .pas source files.
//!
//! This is a pattern-matching parser, not a full Delphi parser. It finds
//! wbRecord(...) calls and parses the nested wb* function calls within them.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;

/// A parsed record definition (top-level wbRecord call).
#[derive(Debug)]
pub struct ParsedRecord {
    pub signature: String,
    pub name: String,
    pub members: Vec<ParsedNode>,
    pub line_number: usize,
}

/// A parsed wb* node -- any nested definition call.
#[derive(Debug, Clone)]
pub enum ParsedNode {
    /// wbStruct(SIG, 'name', [ fields... ])
    Struct {
        signature: Option<String>,
        name: String,
        fields: Vec<ParsedNode>,
    },
    /// wbArray(SIG, 'name', element)
    Array {
        signature: Option<String>,
        name: String,
        element: Box<ParsedNode>,
    },
    /// wbInteger(SIG, 'name', itXX, ...)
    Integer {
        signature: Option<String>,
        name: String,
        int_type: String,
        format: Option<IntegerFormat>,
    },
    /// wbFloat(SIG, 'name')
    Float {
        signature: Option<String>,
        name: String,
    },
    /// wbString(SIG, 'name') / wbLString / wbLenString
    String_ {
        signature: Option<String>,
        name: String,
        kind: StringKind,
    },
    /// wbFormID(SIG, 'name') / wbFormIDCk(SIG, 'name', [refs])
    FormId {
        signature: Option<String>,
        name: String,
        valid_refs: Vec<String>,
    },
    /// wbByteArray(SIG, 'name', size)
    ByteArray {
        signature: Option<String>,
        name: String,
        size: usize,
    },
    /// wbUnion(SIG, 'name', decider, [members])
    Union {
        signature: Option<String>,
        name: String,
        members: Vec<ParsedNode>,
    },
    /// wbEmpty(SIG, 'name')
    Empty {
        signature: Option<String>,
        name: String,
    },
    /// Reference to a predefined variable (e.g., wbEDID, wbFULL)
    VarRef {
        name: String,
    },
    /// wbUnused(count)
    Unused {
        size: usize,
    },
    /// wbRStruct('name', [members])
    RStruct {
        name: String,
        members: Vec<ParsedNode>,
    },
    /// wbRArray('name', element)
    RArray {
        name: String,
        element: Box<ParsedNode>,
    },
    /// Something we could not parse
    Unrecognized {
        text: String,
        line_number: usize,
    },
}

#[derive(Debug, Clone)]
pub enum IntegerFormat {
    Flags(Vec<(Option<u64>, String)>),
    Enum(Vec<(Option<i64>, String)>),
    EnumNamed(String),
}

#[derive(Debug, Clone)]
pub enum StringKind {
    Normal,
    Localized,
    LenPrefixed,
}

/// Top-level parser: finds all wbRecord(...) definitions in source text.
pub fn parse_records(source: &str) -> Vec<ParsedRecord> {
    let mut records = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("wbRecord(") {
            // Find the full extent of this wbRecord call by matching parens/brackets
            let start_offset = source_offset_of_line(source, i);
            if let Some(end_offset) = find_balanced_end(source, start_offset) {
                let full_text = &source[start_offset..=end_offset];
                match parse_record_call(full_text, i + 1) {
                    Ok(rec) => records.push(rec),
                    Err(e) => {
                        eprintln!("WARNING: Failed to parse wbRecord at line {}: {}", i + 1, e);
                    }
                }
                // Skip past this record
                let end_line = source[..=end_offset].lines().count();
                i = end_line;
                continue;
            }
        }
        i += 1;
    }

    records
}

fn source_offset_of_line(source: &str, line_idx: usize) -> usize {
    let mut offset = 0;
    for (idx, line) in source.lines().enumerate() {
        if idx == line_idx {
            // Skip leading whitespace to get to the actual content
            let trimmed_start = line.len() - line.trim_start().len();
            return offset + trimmed_start;
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

/// Find the matching closing paren/bracket for a call starting at `start`.
fn find_balanced_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_string = false;
    let mut in_comment = false;
    let mut in_line_comment = false;
    let mut in_paren_comment = false;
    let mut started = false;
    let mut i = start;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        // Handle line comments
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        // Handle block comments { }
        if in_comment {
            if ch == '}' {
                in_comment = false;
            }
            i += 1;
            continue;
        }

        // Handle (* *) block comments
        if in_paren_comment {
            if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] == b')' {
                in_paren_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        // Handle // line comments
        if !in_string && ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            in_line_comment = true;
            i += 2;
            continue;
        }

        // Handle { block comments
        if !in_string && ch == '{' {
            in_comment = true;
            i += 1;
            continue;
        }

        // Handle (* block comments
        if !in_string && ch == '(' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_paren_comment = true;
            i += 2;
            continue;
        }

        // Handle strings
        if ch == '\'' {
            if in_string {
                // Check for escaped quote ''
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_string = false;
            } else {
                in_string = true;
            }
            i += 1;
            continue;
        }

        if in_string {
            i += 1;
            continue;
        }

        match ch {
            '(' => {
                depth_paren += 1;
                started = true;
            }
            ')' => {
                depth_paren -= 1;
                if started && depth_paren == 0 && depth_bracket == 0 {
                    // Skip any trailing method calls like .SetSummaryKey([1])
                    let mut j = i + 1;
                    while j < bytes.len() {
                        let c = bytes[j] as char;
                        if c == '.' {
                            // Skip .MethodName(...)
                            j += 1;
                            // skip identifier
                            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                                j += 1;
                            }
                            if j < bytes.len() && bytes[j] == b'(' {
                                if let Some(end) = find_balanced_end(source, j) {
                                    j = end + 1;
                                    continue;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    // Look for the semicolon
                    while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b';' {
                        return Some(j);
                    }
                    return Some(i);
                }
            }
            '[' => {
                depth_bracket += 1;
            }
            ']' => {
                depth_bracket -= 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parse a complete wbRecord(...) call text.
fn parse_record_call(text: &str, line_number: usize) -> Result<ParsedRecord> {
    // wbRecord(SIG, 'name', flags_or_nil, [ members... ], ...);
    let inner = extract_call_args(text, "wbRecord")
        .context("Failed to extract wbRecord arguments")?;

    let args = split_top_level_args(&inner);
    if args.len() < 2 {
        bail!("wbRecord needs at least 2 args, got {}", args.len());
    }

    let signature = args[0].trim().to_string();
    let name = extract_string_literal(args[1].trim())
        .unwrap_or_else(|| args[1].trim().to_string());

    // Find the array argument (the [ ... ] with member definitions)
    let mut members = Vec::new();
    for arg in &args[2..] {
        let trimmed = arg.trim();
        if trimmed.starts_with('[') {
            let inner_list = &trimmed[1..trimmed.len()-1];
            members = parse_member_list(inner_list, line_number);
            break;
        }
    }

    Ok(ParsedRecord {
        signature,
        name,
        members,
        line_number,
    })
}

/// Extract the inner content of a function call: `funcName(...)` -> `...`
fn extract_call_args<'a>(text: &'a str, func_name: &str) -> Option<&'a str> {
    let start = text.find(&format!("{}(", func_name))?;
    let paren_start = start + func_name.len();
    let bytes = text.as_bytes();

    let mut depth = 0i32;
    let mut in_string = false;
    let mut in_comment = false;
    let mut in_line_comment = false;
    let mut in_paren_comment = false;
    let mut content_start = None;
    let mut i = paren_start;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_line_comment {
            if ch == '\n' { in_line_comment = false; }
            i += 1;
            continue;
        }
        if in_comment {
            if ch == '}' { in_comment = false; }
            i += 1;
            continue;
        }
        if in_paren_comment {
            if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] == b')' {
                in_paren_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if !in_string && ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if !in_string && ch == '{' {
            in_comment = true;
            i += 1;
            continue;
        }
        if !in_string && ch == '(' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_paren_comment = true;
            i += 2;
            continue;
        }
        if ch == '\'' {
            if in_string {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_string = false;
            } else {
                in_string = true;
            }
            i += 1;
            continue;
        }
        if in_string { i += 1; continue; }

        match ch {
            '(' => {
                if depth == 0 {
                    content_start = Some(i + 1);
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[content_start?..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split arguments at the top level (depth 0) by commas.
/// Respects parentheses, brackets, and string literals.
fn split_top_level_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let bytes = text.as_bytes();
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_string = false;
    let mut in_comment = false;
    let mut in_line_comment = false;
    let mut in_paren_comment = false;
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_line_comment {
            if ch == '\n' { in_line_comment = false; }
            // Don't push comment text into the argument buffer
            i += 1;
            continue;
        }
        if in_comment {
            if ch == '}' { in_comment = false; }
            // skip comment chars from current
            i += 1;
            continue;
        }
        if in_paren_comment {
            if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] == b')' {
                in_paren_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if !in_string && ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if !in_string && ch == '{' {
            in_comment = true;
            i += 1;
            continue;
        }
        if !in_string && ch == '(' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_paren_comment = true;
            i += 2;
            continue;
        }
        if ch == '\'' {
            if in_string {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    current.push('\'');
                    current.push('\'');
                    i += 2;
                    continue;
                }
                in_string = false;
            } else {
                in_string = true;
            }
            current.push(ch);
            i += 1;
            continue;
        }
        if in_string {
            current.push(ch);
            i += 1;
            continue;
        }

        match ch {
            '(' => { depth_paren += 1; current.push(ch); }
            ')' => { depth_paren -= 1; current.push(ch); }
            '[' => { depth_bracket += 1; current.push(ch); }
            ']' => { depth_bracket -= 1; current.push(ch); }
            ',' if depth_paren == 0 && depth_bracket == 0 => {
                args.push(current.clone());
                current.clear();
            }
            _ => { current.push(ch); }
        }
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        args.push(current);
    }
    args
}

/// Parse a comma-separated list of member definitions inside [ ... ].
fn parse_member_list(text: &str, base_line: usize) -> Vec<ParsedNode> {
    let args = split_top_level_args(text);
    let mut members = Vec::new();

    for arg in &args {
        let trimmed = arg.trim();
        if trimmed.is_empty() || trimmed == "nil" {
            continue;
        }
        // Skip stray comments that leaked through as args
        if is_comment_text(trimmed) {
            continue;
        }
        members.push(parse_node(trimmed, base_line));
    }

    members
}

/// Heuristic: detect text that is a stray comment, not a real wb* call or identifier.
/// Comments are plain English text without wb prefix, parens, or identifier structure.
fn is_comment_text(text: &str) -> bool {
    let t = text.trim();
    // Empty or nil
    if t.is_empty() { return true; }
    // If it starts with a known wb pattern or is a valid identifier reference, it's not a comment
    if t.starts_with("wb") || t.starts_with("Is") { return false; }
    // If it contains parens or brackets, it's probably code
    if t.contains('(') || t.contains('[') { return false; }
    // If it's a signature (e.g., WEAP, DATA), not a comment
    if is_signature(t) { return false; }
    // If it's all identifier chars, it could be a variable reference
    if t.chars().all(|c| c.is_alphanumeric() || c == '_') { return false; }
    // Otherwise it's likely a stray comment
    true
}

/// Parse a single wb* node from text.
fn parse_node(text: &str, base_line: usize) -> ParsedNode {
    let trimmed = text.trim();

    // Strip trailing method calls like .SetRequired, .IncludeFlag(...), .SetSummaryKey(...)
    let trimmed = strip_method_chains(trimmed);

    // Check for IsSSE(...) wrapper -- take first branch
    if trimmed.starts_with("IsSSE(") {
        if let Some(inner) = extract_call_args(trimmed, "IsSSE") {
            let branches = split_top_level_args(inner);
            if !branches.is_empty() {
                let first = branches[0].trim();
                if first != "nil" {
                    return parse_node(first, base_line);
                }
                if branches.len() > 1 {
                    let second = branches[1].trim();
                    if second != "nil" {
                        return parse_node(second, base_line);
                    }
                }
            }
        }
    }

    // wbStruct(SIG, 'name', [...])
    if trimmed.starts_with("wbStruct(") || trimmed.starts_with("wbStructSK(") {
        return parse_struct_node(trimmed, base_line);
    }

    // wbArray / wbArrayS
    if trimmed.starts_with("wbArray(") || trimmed.starts_with("wbArrayS(") {
        return parse_array_node(trimmed, base_line);
    }

    // wbRStruct / wbRStructSK / wbRStructExSK
    if trimmed.starts_with("wbRStruct(") || trimmed.starts_with("wbRStructSK(")
        || trimmed.starts_with("wbRStructExSK(")
    {
        return parse_rstruct_node(trimmed, base_line);
    }

    // wbRArray / wbRArrayS
    if trimmed.starts_with("wbRArray(") || trimmed.starts_with("wbRArrayS(") {
        return parse_rarray_node(trimmed, base_line);
    }

    // wbInteger
    if trimmed.starts_with("wbInteger(") {
        return parse_integer_node(trimmed, base_line);
    }

    // wbFloat
    if trimmed.starts_with("wbFloat(") {
        return parse_float_node(trimmed);
    }

    // wbString
    if trimmed.starts_with("wbString(") {
        return parse_string_node(trimmed, StringKind::Normal);
    }
    if trimmed.starts_with("wbLString(") || trimmed.starts_with("wbLStringKC(") {
        return parse_string_node(trimmed, StringKind::Localized);
    }
    if trimmed.starts_with("wbLenString(") {
        return parse_string_node(trimmed, StringKind::LenPrefixed);
    }

    // wbFormID / wbFormIDCk / wbFormIDCkNoReach / wbFormIDCK
    if trimmed.starts_with("wbFormID(") {
        return parse_formid_node(trimmed, false);
    }
    if trimmed.starts_with("wbFormIDCk(")
        || trimmed.starts_with("wbFormIDCK(")
        || trimmed.starts_with("wbFormIDCkNoReach(")
    {
        return parse_formid_node(trimmed, true);
    }

    // wbByteArray
    if trimmed.starts_with("wbByteArray(") {
        return parse_bytearray_node(trimmed);
    }

    // wbUnion
    if trimmed.starts_with("wbUnion(") {
        return parse_union_node(trimmed, base_line);
    }

    // wbEmpty
    if trimmed.starts_with("wbEmpty(") {
        return parse_empty_node(trimmed);
    }

    // wbUnused
    if trimmed.starts_with("wbUnused(") {
        return parse_unused_node(trimmed);
    }

    // wbUnknown(SIG) or wbUnknown(SIG, cpIgnore) -- unknown/unresearched subrecord
    if trimmed.starts_with("wbUnknown(") {
        return parse_unknown_node(trimmed);
    }

    // wbFlagsList(SIG, 'name', [...])
    if trimmed.starts_with("wbFlagsList(") {
        return parse_flagslist_node(trimmed, base_line);
    }

    // wbFlags -- standalone (rare at member level, but possible)
    if trimmed.starts_with("wbFlags(") {
        // Treat as unrecognized at member level
    }

    // Variable references: wbEDID, wbFULL, wbDEST, wbOBND(True), etc.
    // These are identifiers or simple calls to predefined variables/functions.
    if trimmed.starts_with("wb") {
        // Could be wbEDID, wbFULL, wbGenericModel, wbOBND(True), etc.
        let ident = trimmed.split(|c: char| !c.is_alphanumeric() && c != '_')
            .next()
            .unwrap_or(trimmed);
        if ident.len() >= 3 && ident.chars().skip(2).next().map(|c| c.is_uppercase()).unwrap_or(false) {
            return ParsedNode::VarRef {
                name: ident.to_string(),
            };
        }
    }

    // Check for simple identifier references
    let ident = trimmed.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or(trimmed);
    if ident == trimmed || (trimmed.starts_with(ident) && trimmed[ident.len()..].trim_start().starts_with('(')) {
        if ident.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return ParsedNode::VarRef {
                name: ident.to_string(),
            };
        }
    }

    ParsedNode::Unrecognized {
        text: truncate(trimmed, 120).to_string(),
        line_number: base_line,
    }
}

fn parse_struct_node(text: &str, base_line: usize) -> ParsedNode {
    let func_name = if text.starts_with("wbStructSK(") { "wbStructSK" } else { "wbStruct" };
    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);

    // wbStructSK has sort key array as first arg, skip it
    let offset = if func_name == "wbStructSK" { 1 } else { 0 };

    if args.len() < offset + 2 {
        return unrecognized(text, base_line);
    }

    let sig_arg = args[offset].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), offset + 1)
    } else if let Some(_s) = extract_string_literal(sig_arg) {
        // Sometimes name comes first without signature
        (None, offset)
    } else {
        (Some(sig_arg.to_string()), offset + 1)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    // Find the array arg
    let mut fields = Vec::new();
    for arg in &args[name_idx + 1..] {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner_list = &t[1..t.len()-1];
            fields = parse_member_list(inner_list, base_line);
            break;
        }
    }

    ParsedNode::Struct {
        signature,
        name,
        fields,
    }
}

fn parse_array_node(text: &str, base_line: usize) -> ParsedNode {
    let func_name = if text.starts_with("wbArrayS(") { "wbArrayS" } else { "wbArray" };
    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);
    if args.len() < 2 {
        return unrecognized(text, base_line);
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    let element = if name_idx + 1 < args.len() {
        parse_node(args[name_idx + 1].trim(), base_line)
    } else {
        ParsedNode::Unrecognized { text: "missing element".to_string(), line_number: base_line }
    };

    ParsedNode::Array {
        signature,
        name,
        element: Box::new(element),
    }
}

fn parse_rstruct_node(text: &str, base_line: usize) -> ParsedNode {
    let func_name = if text.starts_with("wbRStructExSK(") {
        "wbRStructExSK"
    } else if text.starts_with("wbRStructSK(") {
        "wbRStructSK"
    } else {
        "wbRStruct"
    };

    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);

    // wbRStructSK has sort key as first arg, wbRStructExSK has two extra sort keys
    let offset = match func_name {
        "wbRStructExSK" => 2,
        "wbRStructSK" => 1,
        _ => 0,
    };

    if args.len() < offset + 2 {
        return unrecognized(text, base_line);
    }

    let name = extract_string_literal(args[offset].trim()).unwrap_or_default();

    let mut members = Vec::new();
    for arg in &args[offset + 1..] {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner_list = &t[1..t.len()-1];
            members = parse_member_list(inner_list, base_line);
            break;
        }
    }

    ParsedNode::RStruct {
        name,
        members,
    }
}

fn parse_rarray_node(text: &str, base_line: usize) -> ParsedNode {
    let func_name = if text.starts_with("wbRArrayS(") { "wbRArrayS" } else { "wbRArray" };
    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);
    if args.len() < 2 {
        return unrecognized(text, base_line);
    }

    let name = extract_string_literal(args[0].trim()).unwrap_or_default();
    let element = parse_node(args[1].trim(), base_line);

    ParsedNode::RArray {
        name,
        element: Box::new(element),
    }
}

fn parse_integer_node(text: &str, base_line: usize) -> ParsedNode {
    let inner = match extract_call_args(text, "wbInteger") {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);
    if args.len() < 2 {
        return unrecognized(text, base_line);
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else if extract_string_literal(sig_arg).is_some() {
        (None, 0)
    } else {
        (Some(sig_arg.to_string()), 1)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    let int_type = if name_idx + 1 < args.len() {
        args[name_idx + 1].trim().to_string()
    } else {
        "itU32".to_string()
    };

    // Check for format arg (wbFlags, wbEnum, or named enum ref)
    let format = if name_idx + 2 < args.len() {
        let fmt_arg = args[name_idx + 2].trim();
        parse_integer_format(fmt_arg, base_line)
    } else {
        None
    };

    ParsedNode::Integer {
        signature,
        name,
        int_type,
        format,
    }
}

fn parse_integer_format(text: &str, _base_line: usize) -> Option<IntegerFormat> {
    let trimmed = text.trim();

    // wbFlags([...])
    if trimmed.starts_with("wbFlags(") {
        if let Some(inner) = extract_call_args(trimmed, "wbFlags") {
            let flags = parse_flags_list(inner);
            return Some(IntegerFormat::Flags(flags));
        }
    }

    // wbEnum([...]) or wbEnum([], [...])
    if trimmed.starts_with("wbEnum(") {
        if let Some(inner) = extract_call_args(trimmed, "wbEnum") {
            let values = parse_enum_list(inner);
            return Some(IntegerFormat::Enum(values));
        }
    }

    // Named enum reference like wbWeaponAnimTypeEnum
    if trimmed.starts_with("wb") && trimmed.ends_with("Enum") {
        return Some(IntegerFormat::EnumNamed(trimmed.to_string()));
    }

    // wbDiv, wbBoolEnum, etc. -- treat named references
    if trimmed.starts_with("wb") {
        return Some(IntegerFormat::EnumNamed(trimmed.to_string()));
    }

    None
}

fn parse_flags_list(text: &str) -> Vec<(Option<u64>, String)> {
    let mut flags = Vec::new();
    let args = split_top_level_args(text);

    // The first arg should be the array [...]
    for arg in &args {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner = &t[1..t.len()-1];
            let items = split_top_level_args(inner);
            for (i, item) in items.iter().enumerate() {
                let s = item.trim();
                if let Some(name) = extract_string_literal(s) {
                    flags.push((Some(i as u64), name));
                }
            }
            break;
        }
    }

    flags
}

fn parse_enum_list(text: &str) -> Vec<(Option<i64>, String)> {
    let mut values = Vec::new();
    let args = split_top_level_args(text);

    for arg in &args {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner = &t[1..t.len()-1];
            let items = split_top_level_args(inner);

            // Check if this is a sparse enum (key-value pairs) or sequential
            // Sequential: ['Name1', 'Name2', ...]
            // Sparse: [Ord('s'), 'Short', Ord('l'), 'Long', ...]
            let first = items.first().map(|s| s.trim()).unwrap_or("");
            if first.starts_with("Ord(") || first.parse::<i64>().is_ok() {
                // Sparse enum: alternating key, value
                let mut i = 0;
                while i + 1 < items.len() {
                    let key_str = items[i].trim();
                    let val = extract_string_literal(items[i + 1].trim());
                    let key = parse_enum_key(key_str);
                    if let Some(v) = val {
                        values.push((key, v));
                    }
                    i += 2;
                }
            } else {
                // Sequential enum
                for (i, item) in items.iter().enumerate() {
                    if let Some(name) = extract_string_literal(item.trim()) {
                        values.push((Some(i as i64), name));
                    }
                }
            }
        }
    }

    values
}

fn parse_enum_key(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    if let Ok(v) = trimmed.parse::<i64>() {
        return Some(v);
    }
    if trimmed.starts_with("Ord(") {
        let inner = &trimmed[4..trimmed.len()-1];
        if let Some(s) = extract_string_literal(inner) {
            if let Some(ch) = s.chars().next() {
                return Some(ch as i64);
            }
        }
    }
    None
}

fn parse_float_node(text: &str) -> ParsedNode {
    let inner = match extract_call_args(text, "wbFloat") {
        Some(i) => i,
        None => return ParsedNode::Float { signature: None, name: String::new() },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::Float { signature: None, name: String::new() };
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    ParsedNode::Float { signature, name }
}

fn parse_string_node(text: &str, kind: StringKind) -> ParsedNode {
    let func_name = if text.starts_with("wbLStringKC(") {
        "wbLStringKC"
    } else if text.starts_with("wbLString(") {
        "wbLString"
    } else if text.starts_with("wbLenString(") {
        "wbLenString"
    } else {
        "wbString"
    };

    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return ParsedNode::String_ { signature: None, name: String::new(), kind },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::String_ { signature: None, name: String::new(), kind };
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    ParsedNode::String_ { signature, name, kind }
}

fn parse_formid_node(text: &str, has_check: bool) -> ParsedNode {
    let func_name = if text.starts_with("wbFormIDCkNoReach(") {
        "wbFormIDCkNoReach"
    } else if text.starts_with("wbFormIDCK(") {
        "wbFormIDCK"
    } else if text.starts_with("wbFormIDCk(") {
        "wbFormIDCk"
    } else {
        "wbFormID"
    };

    let inner = match extract_call_args(text, func_name) {
        Some(i) => i,
        None => return ParsedNode::FormId { signature: None, name: String::new(), valid_refs: vec![] },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::FormId { signature: None, name: String::new(), valid_refs: vec![] };
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    let mut valid_refs = Vec::new();
    if has_check {
        // Look for the [...] arg with valid reference signatures
        for arg in &args[name_idx + 1..] {
            let t = arg.trim();
            if t.starts_with('[') {
                let inner = &t[1..t.len()-1];
                let refs = split_top_level_args(inner);
                for r in &refs {
                    let r = r.trim();
                    if is_signature(r) || r == "NULL" {
                        valid_refs.push(r.to_string());
                    }
                }
                break;
            }
        }
    }

    ParsedNode::FormId {
        signature,
        name,
        valid_refs,
    }
}

fn parse_bytearray_node(text: &str) -> ParsedNode {
    let inner = match extract_call_args(text, "wbByteArray") {
        Some(i) => i,
        None => return ParsedNode::ByteArray { signature: None, name: String::new(), size: 0 },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::ByteArray { signature: None, name: String::new(), size: 0 };
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    let size = if name_idx + 1 < args.len() {
        args[name_idx + 1].trim().parse::<usize>().unwrap_or(0)
    } else {
        0
    };

    ParsedNode::ByteArray { signature, name, size }
}

fn parse_union_node(text: &str, base_line: usize) -> ParsedNode {
    let inner = match extract_call_args(text, "wbUnion") {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);
    if args.len() < 2 {
        return unrecognized(text, base_line);
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    // Skip decider arg, find the array
    let mut members = Vec::new();
    for arg in &args[name_idx + 1..] {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner_list = &t[1..t.len()-1];
            members = parse_member_list(inner_list, base_line);
            break;
        }
    }

    ParsedNode::Union {
        signature,
        name,
        members,
    }
}

fn parse_empty_node(text: &str) -> ParsedNode {
    let inner = match extract_call_args(text, "wbEmpty") {
        Some(i) => i,
        None => return ParsedNode::Empty { signature: None, name: String::new() },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::Empty { signature: None, name: String::new() };
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    ParsedNode::Empty { signature, name }
}

fn parse_unused_node(text: &str) -> ParsedNode {
    let inner = match extract_call_args(text, "wbUnused") {
        Some(i) => i,
        None => return ParsedNode::Unused { size: 0 },
    };

    let trimmed = inner.trim();
    let size = trimmed.parse::<usize>().unwrap_or(0);
    ParsedNode::Unused { size }
}

fn parse_unknown_node(text: &str) -> ParsedNode {
    let inner = match extract_call_args(text, "wbUnknown") {
        Some(i) => i,
        None => return ParsedNode::ByteArray { signature: None, name: "Unknown".to_string(), size: 0 },
    };

    let args = split_top_level_args(inner);
    if args.is_empty() {
        return ParsedNode::ByteArray { signature: None, name: "Unknown".to_string(), size: 0 };
    }

    let sig_arg = args[0].trim();
    let signature = if is_signature(sig_arg) {
        Some(sig_arg.to_string())
    } else {
        None
    };

    ParsedNode::ByteArray {
        signature,
        name: "Unknown".to_string(),
        size: 0,
    }
}

fn parse_flagslist_node(text: &str, base_line: usize) -> ParsedNode {
    let inner = match extract_call_args(text, "wbFlagsList") {
        Some(i) => i,
        None => return unrecognized(text, base_line),
    };

    let args = split_top_level_args(inner);
    if args.len() < 2 {
        return unrecognized(text, base_line);
    }

    let sig_arg = args[0].trim();
    let (signature, name_idx) = if is_signature(sig_arg) {
        (Some(sig_arg.to_string()), 1)
    } else {
        (None, 0)
    };

    let name = if name_idx < args.len() {
        extract_string_literal(args[name_idx].trim()).unwrap_or_default()
    } else {
        String::new()
    };

    // Parse flags from the array arg
    let mut flags = Vec::new();
    for arg in &args[name_idx + 1..] {
        let t = arg.trim();
        if t.starts_with('[') {
            let inner_list = &t[1..t.len()-1];
            let items = split_top_level_args(inner_list);
            // wbFlagsList uses sparse format: bit_index, 'Name', bit_index, 'Name', ...
            let mut i = 0;
            while i + 1 < items.len() {
                let bit = items[i].trim().parse::<u64>().ok();
                let label = extract_string_literal(items[i + 1].trim());
                if let Some(l) = label {
                    flags.push((bit, l));
                }
                i += 2;
            }
            break;
        }
    }

    ParsedNode::Integer {
        signature,
        name,
        int_type: "itU32".to_string(),
        format: Some(IntegerFormat::Flags(flags)),
    }
}

// -- Utility functions --

fn extract_string_literal(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len()-1];
        // Handle escaped quotes
        Some(inner.replace("''", "'"))
    } else {
        None
    }
}

fn is_signature(text: &str) -> bool {
    let trimmed = text.trim();
    // Signatures are 4 uppercase letters/digits/underscores, e.g., WEAP, DATA, NAM7, NPC_
    if trimmed.len() < 3 || trimmed.len() > 5 {
        return false;
    }
    if trimmed == "True" || trimmed == "False" || trimmed == "nil" {
        return false;
    }
    // Must start with uppercase letter
    let first = trimmed.chars().next().unwrap();
    if !first.is_ascii_uppercase() {
        return false;
    }
    trimmed.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn strip_method_chains(text: &str) -> &str {
    // Find the end of the main call (matching the outermost parens)
    // then strip anything after (.SetRequired, .IncludeFlag, etc.)
    let bytes = text.as_bytes();
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_string = false;
    let mut in_comment = false;
    let mut in_line_comment = false;
    let mut in_paren_comment = false;
    let mut main_end = text.len();
    let mut i = 0;

    // Find the function name
    while i < bytes.len() && bytes[i] != b'(' {
        i += 1;
    }
    if i >= bytes.len() {
        return text; // No parens, return as-is (variable ref)
    }

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_line_comment {
            if ch == '\n' { in_line_comment = false; }
            i += 1;
            continue;
        }
        if in_comment {
            if ch == '}' { in_comment = false; }
            i += 1;
            continue;
        }
        if in_paren_comment {
            if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] == b')' {
                in_paren_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if !in_string && ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if !in_string && ch == '{' {
            in_comment = true;
            i += 1;
            continue;
        }
        if !in_string && ch == '(' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_paren_comment = true;
            i += 2;
            continue;
        }
        if ch == '\'' {
            if in_string {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_string = false;
            } else {
                in_string = true;
            }
            i += 1;
            continue;
        }
        if in_string { i += 1; continue; }

        match ch {
            '(' => { depth_paren += 1; }
            ')' => {
                depth_paren -= 1;
                if depth_paren == 0 && depth_bracket == 0 {
                    main_end = i + 1;
                    break;
                }
            }
            '[' => { depth_bracket += 1; }
            ']' => { depth_bracket -= 1; }
            _ => {}
        }
        i += 1;
    }

    &text[..main_end]
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

fn unrecognized(text: &str, line_number: usize) -> ParsedNode {
    ParsedNode::Unrecognized {
        text: truncate(text, 120).to_string(),
        line_number,
    }
}

/// Collect statistics about parsed records.
pub fn print_stats(records: &[ParsedRecord]) {
    let mut pattern_counts: HashMap<&str, usize> = HashMap::new();
    let mut unrecognized_count = 0;

    fn count_nodes<'a>(node: &'a ParsedNode, counts: &mut HashMap<&'a str, usize>, unrec: &mut usize) {
        match node {
            ParsedNode::Struct { fields, .. } => {
                *counts.entry("wbStruct").or_default() += 1;
                for f in fields { count_nodes(f, counts, unrec); }
            }
            ParsedNode::Array { element, .. } => {
                *counts.entry("wbArray").or_default() += 1;
                count_nodes(element, counts, unrec);
            }
            ParsedNode::Integer { .. } => { *counts.entry("wbInteger").or_default() += 1; }
            ParsedNode::Float { .. } => { *counts.entry("wbFloat").or_default() += 1; }
            ParsedNode::String_ { .. } => { *counts.entry("wbString").or_default() += 1; }
            ParsedNode::FormId { .. } => { *counts.entry("wbFormID").or_default() += 1; }
            ParsedNode::ByteArray { .. } => { *counts.entry("wbByteArray").or_default() += 1; }
            ParsedNode::Union { members, .. } => {
                *counts.entry("wbUnion").or_default() += 1;
                for m in members { count_nodes(m, counts, unrec); }
            }
            ParsedNode::Empty { .. } => { *counts.entry("wbEmpty").or_default() += 1; }
            ParsedNode::VarRef { .. } => { *counts.entry("VarRef").or_default() += 1; }
            ParsedNode::Unused { .. } => { *counts.entry("wbUnused").or_default() += 1; }
            ParsedNode::RStruct { members, .. } => {
                *counts.entry("wbRStruct").or_default() += 1;
                for m in members { count_nodes(m, counts, unrec); }
            }
            ParsedNode::RArray { element, .. } => {
                *counts.entry("wbRArray").or_default() += 1;
                count_nodes(element, counts, unrec);
            }
            ParsedNode::Unrecognized { .. } => { *unrec += 1; }
        }
    }

    for rec in records {
        *pattern_counts.entry("wbRecord").or_default() += 1;
        for member in &rec.members {
            count_nodes(member, &mut pattern_counts, &mut unrecognized_count);
        }
    }

    eprintln!("\n=== Parse Statistics ===");
    let mut sorted: Vec<_> = pattern_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (pattern, count) in &sorted {
        eprintln!("  {:20} {}", pattern, count);
    }
    eprintln!("  {:20} {}", "UNRECOGNIZED", unrecognized_count);
    eprintln!("========================\n");
}
