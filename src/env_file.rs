use std::path::Path;

use anyhow::{bail, Result};

/// Parse a `.env` file into key-value pairs.
pub fn parse_env_file(path: &Path) -> Result<Vec<(String, String)>> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read env file {}: {}", path.display(), e))?;
    parse_env_contents(&contents)
}

/// Parse `.env`-formatted content into key-value pairs.
///
/// Supports:
/// - `KEY=VALUE` (basic assignment)
/// - `KEY="VALUE"` / `KEY='VALUE'` (quoted values, quotes stripped)
/// - Lines starting with `#` (comments, ignored)
/// - Empty lines (ignored)
pub fn parse_env_contents(contents: &str) -> Result<Vec<(String, String)>> {
    let mut entries = Vec::new();

    for (line_num, line) in contents.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some(eq_pos) = trimmed.find('=') else {
            bail!(
                "malformed env entry on line {}: missing '=' in {:?}",
                line_num + 1,
                trimmed
            );
        };

        let key = trimmed[..eq_pos].trim().to_string();
        let raw_value = trimmed[eq_pos + 1..].trim();
        let value = strip_quotes(raw_value);

        entries.push((key, value));
    }

    Ok(entries)
}

fn strip_quotes(s: &str) -> String {
    if s.len() >= 2 {
        if (s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\''))
        {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_key_value() {
        let result = parse_env_contents("KEY=value").unwrap();
        assert_eq!(result, vec![("KEY".into(), "value".into())]);
    }

    #[test]
    fn parse_double_quoted_value() {
        let result = parse_env_contents("KEY=\"value\"").unwrap();
        assert_eq!(result, vec![("KEY".into(), "value".into())]);
    }

    #[test]
    fn parse_single_quoted_value() {
        let result = parse_env_contents("KEY='value'").unwrap();
        assert_eq!(result, vec![("KEY".into(), "value".into())]);
    }

    #[test]
    fn parse_skips_comments() {
        let result = parse_env_contents("# this is a comment").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_skips_empty_lines() {
        let input = "\n\nKEY=v\n\n";
        let result = parse_env_contents(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("KEY".into(), "v".into()));
    }

    #[test]
    fn parse_op_reference() {
        let result =
            parse_env_contents("KEY=\"op://vault/item/field\"").unwrap();
        assert_eq!(
            result,
            vec![("KEY".into(), "op://vault/item/field".into())]
        );
    }

    #[test]
    fn parse_unquoted_with_special_chars() {
        let result = parse_env_contents(
            "DEBUG=electron-notarize*,electron-osx-sign*,electron-builder:*",
        )
        .unwrap();
        assert_eq!(
            result,
            vec![(
                "DEBUG".into(),
                "electron-notarize*,electron-osx-sign*,electron-builder:*".into()
            )]
        );
    }

    #[test]
    fn parse_errors_on_malformed_line() {
        let result = parse_env_contents("NOEQUALS");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("malformed"));
        assert!(err.contains("line 1"));
    }

    #[test]
    fn parse_value_with_equals() {
        let result = parse_env_contents("KEY=\"a=b\"").unwrap();
        assert_eq!(result, vec![("KEY".into(), "a=b".into())]);
    }

    #[test]
    fn parse_empty_value() {
        let result = parse_env_contents("KEY=").unwrap();
        assert_eq!(result, vec![("KEY".into(), "".into())]);
    }

    #[test]
    fn parse_real_env_tpl() {
        let input = r#"AWS_REGION="op://Development/AWS API Key for Inkdrop dev/region"
AWS_ACCESS_KEY_ID="op://Development/AWS API Key for Inkdrop dev/access key id"
# This is a comment
DEBUG=electron-notarize*,electron-osx-sign*,electron-builder:*"#;

        let result = parse_env_contents(input).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "AWS_REGION");
        assert_eq!(
            result[0].1,
            "op://Development/AWS API Key for Inkdrop dev/region"
        );
        assert_eq!(result[2].0, "DEBUG");
        assert_eq!(
            result[2].1,
            "electron-notarize*,electron-osx-sign*,electron-builder:*"
        );
    }
}
