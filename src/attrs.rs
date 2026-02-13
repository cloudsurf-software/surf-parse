use crate::types::{AttrValue, Attrs, Span};
use crate::error::ParseError;

/// Parse a SurfDoc attribute string into an ordered map.
///
/// Accepted formats:
///   - `[key=value key2="quoted with spaces" flag num=42]`
///   - `key=value key2="quoted"` (without brackets)
///
/// Boolean flags (bare keys without `=`) are stored as `AttrValue::Bool(true)`.
/// Numeric values are stored as `AttrValue::Number`. Everything else is
/// `AttrValue::String`.
pub fn parse_attrs(input: &str) -> Result<Attrs, ParseError> {
    let trimmed = input.trim();

    // Strip surrounding brackets if present.
    let inner = if trimmed.starts_with('[') && trimmed.ends_with(']') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let chars: Vec<char> = inner.chars().collect();
    let len = chars.len();
    let mut pos = 0;
    let mut attrs = Attrs::new();

    while pos < len {
        // Skip whitespace.
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= len {
            break;
        }

        // Scan key: alphanumeric, hyphen, underscore.
        let key_start = pos;
        while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '-' || chars[pos] == '_')
        {
            pos += 1;
        }

        if pos == key_start {
            return Err(ParseError::InvalidAttrs {
                message: format!("unexpected character '{}' at position {}", chars[pos], pos),
                span: Span {
                    start_line: 0,
                    end_line: 0,
                    start_offset: pos,
                    end_offset: pos + 1,
                },
            });
        }

        let key: String = chars[key_start..pos].iter().collect();

        // Check for `=`.
        if pos < len && chars[pos] == '=' {
            pos += 1; // consume `=`

            if pos >= len {
                return Err(ParseError::InvalidAttrs {
                    message: format!("missing value after '=' for key '{key}'"),
                    span: Span {
                        start_line: 0,
                        end_line: 0,
                        start_offset: pos,
                        end_offset: pos,
                    },
                });
            }

            if chars[pos] == '"' {
                // Quoted value.
                pos += 1; // consume opening quote
                let mut value = String::new();
                while pos < len && chars[pos] != '"' {
                    if chars[pos] == '\\' && pos + 1 < len {
                        let next = chars[pos + 1];
                        match next {
                            '"' | '\\' => {
                                value.push(next);
                                pos += 2;
                            }
                            _ => {
                                value.push(chars[pos]);
                                pos += 1;
                            }
                        }
                    } else {
                        value.push(chars[pos]);
                        pos += 1;
                    }
                }
                if pos < len && chars[pos] == '"' {
                    pos += 1; // consume closing quote
                } else {
                    return Err(ParseError::InvalidAttrs {
                        message: format!("unterminated quoted value for key '{key}'"),
                        span: Span {
                            start_line: 0,
                            end_line: 0,
                            start_offset: key_start,
                            end_offset: pos,
                        },
                    });
                }
                attrs.insert(key, AttrValue::String(value));
            } else {
                // Unquoted value: read until whitespace or `]`.
                let val_start = pos;
                while pos < len && !chars[pos].is_whitespace() && chars[pos] != ']' {
                    pos += 1;
                }
                let raw: String = chars[val_start..pos].iter().collect();
                attrs.insert(key, coerce_value(&raw));
            }
        } else {
            // No `=` — boolean flag.
            attrs.insert(key, AttrValue::Bool(true));
        }
    }

    Ok(attrs)
}

/// Coerce an unquoted value string into the most specific `AttrValue`:
/// `true`/`false` -> Bool, valid f64 -> Number, otherwise String.
fn coerce_value(raw: &str) -> AttrValue {
    match raw {
        "true" => AttrValue::Bool(true),
        "false" => AttrValue::Bool(false),
        "null" => AttrValue::Null,
        _ => {
            if let Ok(n) = raw.parse::<f64>() {
                // Avoid coercing things like `v1.2` (parse would fail anyway).
                AttrValue::Number(n)
            } else {
                AttrValue::String(raw.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_attrs() {
        let attrs = parse_attrs("[]").unwrap();
        assert!(attrs.is_empty());
    }

    #[test]
    fn parse_single_unquoted() {
        let attrs = parse_attrs("[key=value]").unwrap();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs["key"], AttrValue::String("value".into()));
    }

    #[test]
    fn parse_quoted_value() {
        let attrs = parse_attrs(r#"[key="hello world"]"#).unwrap();
        assert_eq!(attrs["key"], AttrValue::String("hello world".into()));
    }

    #[test]
    fn parse_boolean_flag() {
        let attrs = parse_attrs("[sortable]").unwrap();
        assert_eq!(attrs["sortable"], AttrValue::Bool(true));
    }

    #[test]
    fn parse_numeric() {
        let attrs = parse_attrs("[count=42]").unwrap();
        assert_eq!(attrs["count"], AttrValue::Number(42.0));
    }

    #[test]
    fn parse_multiple() {
        let attrs = parse_attrs(r#"[id=x sortable key="val"]"#).unwrap();
        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs["id"], AttrValue::String("x".into()));
        assert_eq!(attrs["sortable"], AttrValue::Bool(true));
        assert_eq!(attrs["key"], AttrValue::String("val".into()));
    }

    #[test]
    fn parse_escaped_quote() {
        let attrs = parse_attrs(r#"[key="say \"hi\""]"#).unwrap();
        assert_eq!(attrs["key"], AttrValue::String(r#"say "hi""#.into()));
    }

    #[test]
    fn parse_no_brackets() {
        let attrs = parse_attrs("key=value").unwrap();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs["key"], AttrValue::String("value".into()));
    }

    #[test]
    fn parse_bool_values() {
        let attrs = parse_attrs("[enabled=true disabled=false]").unwrap();
        assert_eq!(attrs["enabled"], AttrValue::Bool(true));
        assert_eq!(attrs["disabled"], AttrValue::Bool(false));
    }

    #[test]
    fn parse_null_value() {
        let attrs = parse_attrs("[val=null]").unwrap();
        assert_eq!(attrs["val"], AttrValue::Null);
    }

    #[test]
    fn parse_float_number() {
        let attrs = parse_attrs("[ratio=3.14]").unwrap();
        assert_eq!(attrs["ratio"], AttrValue::Number(3.14));
    }
}
