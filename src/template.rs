//! Template variable interpolation for SurfDoc rendering.
//!
//! Provides `{= key =}` variable substitution during HTML output.
//! Variables are resolved at render time (not parse time), keeping the AST clean.
//!
//! # Usage
//!
//! ```
//! use surf_parse::template::TemplateContext;
//!
//! let mut ctx = TemplateContext::new();
//! ctx.insert("name", "Brady");
//! ctx.insert("user.email", "brady@cloudsurf.com");
//!
//! let html = "<h1>Hello, {= name =}!</h1><p>{= user.email =}</p>";
//! let result = ctx.resolve(html);
//! assert_eq!(result, "<h1>Hello, Brady!</h1><p>brady@cloudsurf.com</p>");
//! ```

use std::collections::HashMap;

/// Context for template variable interpolation.
///
/// Variables are flat key-value pairs. Dot-path notation (e.g., `user.name`)
/// is stored and looked up as a flat string key — no nested map traversal.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    vars: HashMap<String, String>,
}

impl TemplateContext {
    /// Create an empty template context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a variable. Returns `&mut Self` for chaining.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    /// Resolve `{= key =}` patterns in an HTML string.
    ///
    /// - Matched keys are replaced with their HTML-escaped value.
    /// - Missing keys are replaced with an empty string.
    /// - Whitespace inside delimiters is trimmed: `{= name =}` and `{=name=}` both work.
    pub fn resolve(&self, html: &str) -> String {
        let mut result = String::with_capacity(html.len());
        let mut rest = html;

        while let Some(start_pos) = rest.find("{=") {
            // Push everything before the delimiter
            result.push_str(&rest[..start_pos]);

            let after_open = &rest[start_pos + 2..];
            if let Some(end_pos) = after_open.find("=}") {
                let key = after_open[..end_pos].trim();
                if let Some(value) = self.vars.get(key) {
                    result.push_str(&escape_html(value));
                }
                // Missing keys → empty string (nothing pushed)
                rest = &after_open[end_pos + 2..];
            } else {
                // No closing =} found — emit the {= literally and move past it
                result.push_str("{=");
                rest = after_open;
            }
        }

        // Push any remaining text
        result.push_str(rest);
        result
    }
}

/// Escape HTML special characters to prevent XSS in interpolated values.
fn escape_html(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_interpolation() {
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "Brady");
        assert_eq!(ctx.resolve("Hello, {= name =}!"), "Hello, Brady!");
    }

    #[test]
    fn dot_path_key() {
        let mut ctx = TemplateContext::new();
        ctx.insert("user.email", "brady@cloudsurf.com");
        assert_eq!(
            ctx.resolve("Email: {= user.email =}"),
            "Email: brady@cloudsurf.com"
        );
    }

    #[test]
    fn missing_key_is_empty() {
        let ctx = TemplateContext::new();
        assert_eq!(ctx.resolve("Hello, {= unknown =}!"), "Hello, !");
    }

    #[test]
    fn html_escaping() {
        let mut ctx = TemplateContext::new();
        ctx.insert("input", "<script>alert(1)</script>");
        assert_eq!(
            ctx.resolve("{= input =}"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn ampersand_escaping() {
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "Tom & Jerry");
        assert_eq!(ctx.resolve("{= name =}"), "Tom &amp; Jerry");
    }

    #[test]
    fn quote_escaping() {
        let mut ctx = TemplateContext::new();
        ctx.insert("attr", "say \"hello\"");
        assert_eq!(ctx.resolve("{= attr =}"), "say &quot;hello&quot;");
    }

    #[test]
    fn no_variables_unchanged() {
        let ctx = TemplateContext::new();
        assert_eq!(
            ctx.resolve("<h1>No variables here</h1>"),
            "<h1>No variables here</h1>"
        );
    }

    #[test]
    fn multiple_variables_one_line() {
        let mut ctx = TemplateContext::new();
        ctx.insert("first", "Brady");
        ctx.insert("last", "Davis");
        assert_eq!(
            ctx.resolve("{= first =} {= last =}"),
            "Brady Davis"
        );
    }

    #[test]
    fn variable_in_html_context() {
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "Brady");
        assert_eq!(
            ctx.resolve("<h1>Hello {= name =}</h1>"),
            "<h1>Hello Brady</h1>"
        );
    }

    #[test]
    fn no_whitespace_in_delimiters() {
        let mut ctx = TemplateContext::new();
        ctx.insert("x", "42");
        assert_eq!(ctx.resolve("{=x=}"), "42");
    }

    #[test]
    fn extra_whitespace_in_delimiters() {
        let mut ctx = TemplateContext::new();
        ctx.insert("x", "42");
        assert_eq!(ctx.resolve("{=   x   =}"), "42");
    }

    #[test]
    fn unclosed_delimiter_is_literal() {
        let ctx = TemplateContext::new();
        assert_eq!(ctx.resolve("price {= no closing"), "price {= no closing");
    }

    #[test]
    fn empty_key_is_missing() {
        let ctx = TemplateContext::new();
        assert_eq!(ctx.resolve("{= =}"), "");
    }

    #[test]
    fn chained_insert() {
        let mut ctx = TemplateContext::new();
        ctx.insert("a", "1").insert("b", "2");
        assert_eq!(ctx.resolve("{= a =},{= b =}"), "1,2");
    }

    #[test]
    fn adjacent_variables() {
        let mut ctx = TemplateContext::new();
        ctx.insert("a", "hello");
        ctx.insert("b", "world");
        assert_eq!(ctx.resolve("{= a =}{= b =}"), "helloworld");
    }

    #[test]
    fn variable_at_start_and_end() {
        let mut ctx = TemplateContext::new();
        ctx.insert("greeting", "Hi");
        assert_eq!(ctx.resolve("{= greeting =}"), "Hi");
    }

    #[test]
    fn preserves_surrounding_text() {
        let mut ctx = TemplateContext::new();
        ctx.insert("plan", "Pro");
        assert_eq!(
            ctx.resolve("Your plan: {= plan =}. Enjoy!"),
            "Your plan: Pro. Enjoy!"
        );
    }
}
