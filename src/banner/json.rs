//! JSON parsing utilities for the Banner API client.

use anyhow::Result;

/// Attempt to parse JSON and, on failure, include a contextual snippet of the
/// line where the error occurred along with the serde path and type mismatch.
pub fn parse_json_with_context<T: serde::de::DeserializeOwned>(body: &str) -> Result<T> {
    let jd = &mut serde_json::Deserializer::from_str(body);
    match serde_path_to_error::deserialize(jd) {
        Ok(value) => Ok(value),
        Err(err) => {
            let inner_err = err.inner();
            let (line, column) = (inner_err.line(), inner_err.column());
            let path = err.path().to_string();

            let msg = inner_err.to_string();
            let loc = format!(" at line {line} column {column}");
            let msg_without_loc = msg.strip_suffix(&loc).unwrap_or(&msg).to_string();

            let type_info = parse_type_mismatch(&msg_without_loc);
            let snippet = build_error_snippet(body, line, column, 20);

            let mut final_err = String::new();
            if !path.is_empty() && path != "." {
                final_err.push_str(&format!("at path '{}': ", path));
            }
            final_err.push_str(&format!(
                "{} (line {} col {})\n{}",
                type_info, line, column, snippet
            ));

            Err(anyhow::anyhow!(final_err))
        }
    }
}

/// Extract type mismatch information from a serde error message.
///
/// Parses error messages like "invalid type: null, expected a string" to extract
/// the expected and actual types for clearer error reporting.
///
/// Returns a formatted string like "expected a string, got null" or the original
/// message if parsing fails.
fn parse_type_mismatch(error_msg: &str) -> String {
    // Try to parse "invalid type: X, expected Y" format
    if let Some(invalid_start) = error_msg.find("invalid type: ") {
        let after_prefix = &error_msg[invalid_start + "invalid type: ".len()..];

        if let Some(comma_pos) = after_prefix.find(", expected ") {
            let actual_type = &after_prefix[..comma_pos];
            let expected_part = &after_prefix[comma_pos + ", expected ".len()..];

            // Clean up expected part (remove " at line X column Y" if present)
            let expected_type = expected_part
                .split(" at line ")
                .next()
                .unwrap_or(expected_part)
                .trim();

            return format!("expected {}, got {}", expected_type, actual_type);
        }
    }

    // Try to parse "expected X at line Y" format
    if error_msg.starts_with("expected ")
        && let Some(expected_part) = error_msg.split(" at line ").next()
    {
        return expected_part.to_string();
    }

    // Fallback: return original message without location info
    error_msg.to_string()
}

fn build_error_snippet(body: &str, line: usize, column: usize, context_len: usize) -> String {
    let target_line = body.lines().nth(line.saturating_sub(1)).unwrap_or("");
    if target_line.is_empty() {
        return "(empty line)".to_string();
    }

    // column is 1-based, convert to 0-based for slicing
    let error_idx = column.saturating_sub(1);

    let half_len = context_len / 2;
    let start = error_idx.saturating_sub(half_len);
    let end = (error_idx + half_len).min(target_line.len());

    let slice = &target_line[start..end];
    let indicator_pos = error_idx - start;

    let indicator = " ".repeat(indicator_pos) + "^";

    format!("...{slice}...\n   {indicator}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_parse_type_mismatch_invalid_type() {
        let msg = "invalid type: null, expected a string at line 45 column 29";
        let result = parse_type_mismatch(msg);
        assert_eq!(result, "expected a string, got null");
    }

    #[test]
    fn test_parse_type_mismatch_expected() {
        let msg = "expected value at line 1 column 1";
        let result = parse_type_mismatch(msg);
        assert_eq!(result, "expected value");
    }

    #[test]
    fn test_parse_json_with_context_null_value() {
        #[derive(Debug, Deserialize)]
        struct TestStruct {
            #[allow(dead_code)]
            name: String,
        }

        let json = r#"{"name": null}"#;
        let result: Result<TestStruct> = parse_json_with_context(json);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();

        assert!(err_msg.contains("name"));
        assert!(err_msg.contains("expected"));
        assert!(err_msg.contains("got"));
    }

    #[test]
    fn test_realistic_banner_error() {
        #[derive(Debug, Deserialize)]
        struct Course {
            #[allow(dead_code)]
            #[serde(rename = "courseTitle")]
            course_title: String,
            #[allow(dead_code)]
            faculty: Vec<Faculty>,
        }

        #[derive(Debug, Deserialize)]
        struct Faculty {
            #[serde(rename = "displayName")]
            #[allow(dead_code)]
            display_name: String,
            #[allow(dead_code)]
            email: String,
        }

        #[derive(Debug, Deserialize)]
        struct SearchResult {
            #[allow(dead_code)]
            data: Vec<Course>,
        }

        let json = r#"{
            "data": [
                {
                    "courseTitle": "Spanish Conversation",
                    "faculty": [
                        {
                            "displayName": null,
                            "email": "instructor@utsa.edu"
                        }
                    ]
                }
            ]
        }"#;

        let result: Result<SearchResult> = parse_json_with_context(json);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        println!("\n=== Error output ===\n{}\n", err_msg);

        assert!(err_msg.contains("data[0].faculty[0].displayName"));
        assert!(err_msg.contains("expected") && err_msg.contains("got"));
    }
}
