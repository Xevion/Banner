//! Custom tracing formatter

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fmt;
use time::macros::format_description;
use time::{OffsetDateTime, format_description::FormatItem};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;
use yansi::Paint;

/// Cached format description for timestamps
const TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[hour]:[minute]:[second].[subsecond digits:5]");

/// Maximum length for string values before truncation
const MAX_VALUE_LENGTH: usize = 60;

/// Characters that require quoting when present in a string value
const DELIMITER_CHARS: &[char] = &['=', '{', '}', ':', ',', ' ', '\t', '\n', '\r'];

/// Truncate a string to at most `max_len` bytes without splitting a multi-byte character.
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Check if a string value needs quoting
fn needs_quoting(s: &str) -> bool {
    s.is_empty() || s.contains(DELIMITER_CHARS)
}

/// Format a string value, adding quotes and truncation as needed
fn format_string_value(s: &str) -> String {
    // Already quoted - pass through (but still truncate if needed)
    if s.starts_with('"') && s.ends_with('"') {
        let inner = &s[1..s.len() - 1];
        if inner.len() > MAX_VALUE_LENGTH {
            return format!("\"{}...\"", truncate_str(inner, MAX_VALUE_LENGTH));
        }
        return s.to_string();
    }

    let needs_quote = needs_quoting(s);
    let needs_truncate = s.len() > MAX_VALUE_LENGTH;

    match (needs_quote, needs_truncate) {
        (false, false) => s.to_string(),
        (false, true) => format!("{}...", truncate_str(s, MAX_VALUE_LENGTH)),
        (true, false) => format!("\"{}\"", escape_string(s)),
        (true, true) => format!(
            "\"{}...\"",
            escape_string(truncate_str(s, MAX_VALUE_LENGTH))
        ),
    }
}

/// Escape special characters in a string for quoting
/// Also strips ANSI escape codes and other control characters that could
/// interfere with terminal rendering
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // ANSI escape sequence - skip the entire sequence
            '\x1b' => {
                // Skip until we hit the end of the escape sequence
                // CSI sequences: ESC [ ... final_byte (0x40-0x7E)
                // OSC sequences: ESC ] ... ST (ESC \ or BEL)
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    // Skip until final byte (@ through ~)
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                } else if chars.peek() == Some(&']') {
                    chars.next(); // consume ']'
                    // Skip until BEL or ST
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Other escape sequences - just skip the next char
                else {
                    chars.next();
                }
            }
            // Other control characters (except the ones we handle above)
            c if c.is_control() => {
                // Represent as \xNN
                result.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// A collected field value for formatting
#[derive(Debug, Clone)]
enum FieldValue {
    Debug(String),
    Display(String),
    Signed(i64),
    Unsigned(u64),
    Bool(bool),
}

trait WriteColored {
    fn write_colored(&self, writer: &mut Writer<'_>) -> fmt::Result;
}

/// Write a string value with appropriate coloring (arrays, numbers, quoted strings).
fn write_str_value_colored(writer: &mut Writer<'_>, s: &str) -> fmt::Result {
    let ansi = writer.has_ansi_escapes();

    if s.starts_with('[') && s.ends_with(']') {
        write_array_colored(writer, s)
    } else if s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() {
        if ansi {
            write!(writer, "{}", Paint::new(s).magenta())
        } else {
            write!(writer, "{}", s)
        }
    } else {
        let formatted = format_string_value(s);
        if formatted.starts_with('"') && ansi {
            write!(writer, "{}", Paint::new(&formatted).yellow())
        } else {
            write!(writer, "{}", formatted)
        }
    }
}

impl WriteColored for FieldValue {
    fn write_colored(&self, writer: &mut Writer<'_>) -> fmt::Result {
        let ansi = writer.has_ansi_escapes();
        match self {
            FieldValue::Debug(s) | FieldValue::Display(s) => write_str_value_colored(writer, s),
            FieldValue::Signed(n) => {
                if ansi {
                    write!(writer, "{}", Paint::new(n).magenta())
                } else {
                    write!(writer, "{}", n)
                }
            }
            FieldValue::Unsigned(n) => {
                if ansi {
                    write!(writer, "{}", Paint::new(n).magenta())
                } else {
                    write!(writer, "{}", n)
                }
            }
            FieldValue::Bool(b) => {
                if ansi {
                    write!(writer, "{}", Paint::new(b).magenta())
                } else {
                    write!(writer, "{}", b)
                }
            }
        }
    }
}

impl WriteColored for String {
    fn write_colored(&self, writer: &mut Writer<'_>) -> fmt::Result {
        write_str_value_colored(writer, self)
    }
}

/// Write an array value with colored elements
/// Input format: `["a", "b", 123]` (Debug-formatted Rust arrays)
fn write_array_colored(writer: &mut Writer<'_>, s: &str) -> fmt::Result {
    // Strip outer brackets
    let inner = &s[1..s.len() - 1];

    write_dim_char(writer, '[')?;

    if !inner.is_empty() {
        // Parse and colorize each element
        let mut remaining = inner.trim();
        let mut first = true;

        while !remaining.is_empty() {
            if !first {
                write_dim_str(writer, ", ")?;
            }
            first = false;

            // Parse next element
            let (element, rest) = parse_array_element(remaining);
            write_array_element_colored(writer, element)?;
            remaining = rest.trim_start();

            // Skip comma if present
            if remaining.starts_with(',') {
                remaining = remaining[1..].trim_start();
            }
        }
    }

    write_dim_char(writer, ']')
}

/// Parse one element from an array string, returning (element, remaining)
fn parse_array_element(s: &str) -> (&str, &str) {
    let s = s.trim_start();

    if s.starts_with('"') {
        // Quoted string - find closing quote (handling escapes)
        let mut end = 1;
        let bytes = s.as_bytes();
        while end < bytes.len() {
            if bytes[end] == b'"' && (end == 1 || bytes[end - 1] != b'\\') {
                return (&s[..=end], &s[end + 1..]);
            }
            end += 1;
        }
        // Malformed - return rest
        (s, "")
    } else if s.starts_with('[') {
        // Nested array - find matching bracket
        let mut depth = 1;
        let mut end = 1;
        let bytes = s.as_bytes();
        while end < bytes.len() && depth > 0 {
            match bytes[end] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        (&s[..end], &s[end..])
    } else {
        // Unquoted value - read until comma or end
        let end = s.find(',').unwrap_or(s.len());
        (s[..end].trim_end(), &s[end..])
    }
}

/// Write a single array element with appropriate coloring
fn write_array_element_colored(writer: &mut Writer<'_>, element: &str) -> fmt::Result {
    let ansi = writer.has_ansi_escapes();

    if element.starts_with('"') && element.ends_with('"') {
        // String element - yellow
        if ansi {
            write!(writer, "{}", Paint::new(element).yellow())
        } else {
            write!(writer, "{}", element)
        }
    } else if element.starts_with('[') && element.ends_with(']') {
        // Nested array - recurse
        write_array_colored(writer, element)
    } else if element.parse::<i64>().is_ok() || element.parse::<u64>().is_ok() {
        // Number - magenta
        if ansi {
            write!(writer, "{}", Paint::new(element).magenta())
        } else {
            write!(writer, "{}", element)
        }
    } else if element == "true" || element == "false" {
        // Boolean - magenta
        if ansi {
            write!(writer, "{}", Paint::new(element).magenta())
        } else {
            write!(writer, "{}", element)
        }
    } else {
        // Other - default
        write!(writer, "{}", element)
    }
}

/// Visitor that collects fields into a map for later formatting
struct FieldCollector {
    fields: IndexMap<String, FieldValue>,
    message: Option<String>,
}

impl FieldCollector {
    fn new() -> Self {
        Self {
            fields: IndexMap::new(),
            message: None,
        }
    }
}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let name = field.name();
        if name == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            self.fields
                .insert(name.to_string(), FieldValue::Debug(format!("{:?}", value)));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields
                .insert(name.to_string(), FieldValue::Display(value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() != "message" {
            self.fields
                .insert(field.name().to_string(), FieldValue::Signed(value));
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() != "message" {
            self.fields
                .insert(field.name().to_string(), FieldValue::Unsigned(value));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() != "message" {
            self.fields
                .insert(field.name().to_string(), FieldValue::Bool(value));
        }
    }
}

/// Groups dotted keys like `thing.a`, `thing.b` into nested groups
fn group_dotted_fields<V>(fields: IndexMap<String, V>) -> IndexMap<String, GroupedField<V>> {
    let mut result: IndexMap<String, GroupedField<V>> = IndexMap::new();

    for (key, value) in fields {
        if let Some((prefix, suffix)) = key.split_once('.') {
            // Dotted key - group it
            match result.get_mut(prefix) {
                Some(GroupedField::Group(map)) => {
                    map.insert(suffix.to_string(), value);
                }
                Some(GroupedField::Single(_)) => {
                    // Conflict: already have a non-grouped field with this name
                    // Keep the original and add this as a separate key
                    result.insert(key, GroupedField::Single(value));
                }
                None => {
                    let mut map = IndexMap::new();
                    map.insert(suffix.to_string(), value);
                    result.insert(prefix.to_string(), GroupedField::Group(map));
                }
            }
        } else {
            // Non-dotted key
            result.insert(key, GroupedField::Single(value));
        }
    }

    result
}

enum GroupedField<V> {
    Single(V),
    Group(IndexMap<String, V>),
}

/// Write grouped fields with colors
fn write_grouped_fields<V: WriteColored>(
    writer: &mut Writer<'_>,
    fields: IndexMap<String, GroupedField<V>>,
) -> fmt::Result {
    let mut first = true;

    for (key, grouped) in fields {
        if !first {
            writer.write_char(' ')?;
        }
        first = false;

        match grouped {
            GroupedField::Single(value) => {
                write_field_key(writer, &key)?;
                write_field_eq(writer)?;
                value.write_colored(writer)?;
            }
            GroupedField::Group(subfields) => {
                write_field_key(writer, &key)?;
                write_field_eq(writer)?;
                write_dim_char(writer, '{')?;

                let mut sub_first = true;
                for (subkey, subvalue) in subfields {
                    if !sub_first {
                        write_dim_str(writer, ", ")?;
                    }
                    sub_first = false;

                    write_field_key(writer, &subkey)?;
                    write_field_eq(writer)?;
                    subvalue.write_colored(writer)?;
                }

                write_dim_char(writer, '}')?;
            }
        }
    }

    Ok(())
}

fn write_dim_char(writer: &mut Writer<'_>, c: char) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new(c).dim())
    } else {
        writer.write_char(c)
    }
}

fn write_dim_str(writer: &mut Writer<'_>, s: &str) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new(s).dim())
    } else {
        writer.write_str(s)
    }
}

fn write_field_key(writer: &mut Writer<'_>, key: &str) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new(key).cyan())
    } else {
        write!(writer, "{}", key)
    }
}

fn write_field_eq(writer: &mut Writer<'_>) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new("=").dim())
    } else {
        writer.write_char('=')
    }
}

/// Parse span fields string and reformat with colors
fn write_span_fields_colored(writer: &mut Writer<'_>, fields_str: &str) -> fmt::Result {
    if fields_str.is_empty() {
        return Ok(());
    }

    // Parse key=value pairs (simple parser, handles quoted values)
    let mut fields: IndexMap<String, String> = IndexMap::new();
    let mut remaining = fields_str;

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // Find the key
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => break,
        };
        let key = remaining[..eq_pos].trim();
        remaining = &remaining[eq_pos + 1..];

        // Find the value (handle quoted strings)
        let value = if remaining.starts_with('"') {
            // Quoted string - find closing quote
            let mut end = 1;
            let chars: Vec<char> = remaining.chars().collect();
            while end < chars.len() {
                if chars[end] == '"' && (end == 0 || chars[end - 1] != '\\') {
                    break;
                }
                end += 1;
            }
            let byte_end = chars[..=end.min(chars.len() - 1)]
                .iter()
                .collect::<String>()
                .len();
            let val = &remaining[..byte_end];
            remaining = &remaining[byte_end..];
            val
        } else {
            // Unquoted - read until space or end
            let space_pos = remaining.find(' ').unwrap_or(remaining.len());
            let val = &remaining[..space_pos];
            remaining = &remaining[space_pos..];
            val
        };

        fields.insert(key.to_string(), value.to_string());
    }

    let grouped = group_dotted_fields(fields);
    write_grouped_fields(writer, grouped)
}

/// A custom formatter with enhanced timestamp formatting and colored fields
///
/// Re-implementation of the Full formatter with improved timestamp display
/// and colored structured logging output.
pub struct CustomPrettyFormatter;

impl<S, N> FormatEvent<S, N> for CustomPrettyFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let ansi = writer.has_ansi_escapes();

        // 1) Timestamp (dimmed when ANSI)
        let now = OffsetDateTime::now_utc();
        let formatted_time = now.format(&TIMESTAMP_FORMAT).map_err(|e| {
            eprintln!("Failed to format timestamp: {}", e);
            fmt::Error
        })?;
        write_dimmed(&mut writer, formatted_time)?;
        writer.write_char(' ')?;

        // 2) Colored 5-char level like Full
        write_colored_level(&mut writer, meta.level())?;
        writer.write_char(' ')?;

        // 3) Span scope chain (bold names, colored fields in braces, dimmed ':')
        if let Some(scope) = ctx.event_scope() {
            let mut saw_any = false;
            for span in scope.from_root() {
                write_bold(&mut writer, span.metadata().name())?;
                saw_any = true;

                let ext = span.extensions();
                if let Some(fields) = ext.get::<FormattedFields<N>>()
                    && !fields.fields.is_empty()
                {
                    write_dim_char(&mut writer, '{')?;
                    write_span_fields_colored(&mut writer, fields.fields.as_str())?;
                    write_dim_char(&mut writer, '}')?;
                }
                write_dimmed(&mut writer, ":")?;
            }

            if saw_any {
                writer.write_char(' ')?;
            }
        }

        // 4) Target (dimmed), then a space
        if ansi {
            write!(writer, "{}: ", Paint::new(meta.target()).dim())?;
        } else {
            write!(writer, "{}: ", meta.target())?;
        }

        // 5) Collect and format event fields with colors
        let mut collector = FieldCollector::new();
        event.record(&mut collector);

        // Write message first if present
        if let Some(msg) = &collector.message {
            write!(writer, "{}", msg)?;
            if !collector.fields.is_empty() {
                writer.write_char(' ')?;
            }
        }

        // Write fields with grouping and colors
        let grouped = group_dotted_fields(collector.fields);
        write_grouped_fields(&mut writer, grouped)?;

        // 6) Newline
        writeln!(writer)
    }
}

/// A custom JSON formatter that flattens fields to root level
///
/// Outputs logs in the format: { "message": "...", "level": "...", "customAttribute": "..." }
pub struct CustomJsonFormatter;

impl<S, N> FormatEvent<S, N> for CustomJsonFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();

        #[derive(Serialize)]
        struct EventFields {
            message: String,
            level: String,
            target: String,
            #[serde(flatten)]
            spans: Map<String, Value>,
            #[serde(flatten)]
            fields: Map<String, Value>,
        }

        let (message, fields, spans) = {
            let mut message: Option<String> = None;
            let mut fields: Map<String, Value> = Map::new();
            let mut spans: Map<String, Value> = Map::new();

            struct FieldVisitor<'a> {
                message: &'a mut Option<String>,
                fields: &'a mut Map<String, Value>,
            }

            impl<'a> Visit for FieldVisitor<'a> {
                fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                    let key = field.name();
                    if key == "message" {
                        *self.message = Some(format!("{:?}", value));
                    } else {
                        // Use typed methods for better performance
                        self.fields
                            .insert(key.to_string(), Value::String(format!("{:?}", value)));
                    }
                }

                fn record_str(&mut self, field: &Field, value: &str) {
                    let key = field.name();
                    if key == "message" {
                        *self.message = Some(value.to_string());
                    } else {
                        self.fields
                            .insert(key.to_string(), Value::String(value.to_string()));
                    }
                }

                fn record_i64(&mut self, field: &Field, value: i64) {
                    let key = field.name();
                    if key != "message" {
                        self.fields.insert(
                            key.to_string(),
                            Value::Number(serde_json::Number::from(value)),
                        );
                    }
                }

                fn record_u64(&mut self, field: &Field, value: u64) {
                    let key = field.name();
                    if key != "message" {
                        self.fields.insert(
                            key.to_string(),
                            Value::Number(serde_json::Number::from(value)),
                        );
                    }
                }

                fn record_bool(&mut self, field: &Field, value: bool) {
                    let key = field.name();
                    if key != "message" {
                        self.fields.insert(key.to_string(), Value::Bool(value));
                    }
                }
            }

            let mut visitor = FieldVisitor {
                message: &mut message,
                fields: &mut fields,
            };
            event.record(&mut visitor);

            // Collect span information from the span hierarchy
            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    let span_name = span.metadata().name().to_string();
                    let mut span_fields: Map<String, Value> = Map::new();

                    // Try to extract fields from FormattedFields
                    let ext = span.extensions();
                    if let Some(formatted_fields) = ext.get::<FormattedFields<N>>() {
                        // Try to parse as JSON first
                        if let Ok(json_fields) = serde_json::from_str::<Map<String, Value>>(
                            formatted_fields.fields.as_str(),
                        ) {
                            span_fields.extend(json_fields);
                        } else {
                            // If not valid JSON, treat the entire field string as a single field
                            span_fields.insert(
                                "raw".to_string(),
                                Value::String(formatted_fields.fields.as_str().to_string()),
                            );
                        }
                    }

                    // Insert span as a nested object directly into the spans map
                    spans.insert(span_name, Value::Object(span_fields));
                }
            }

            (message, fields, spans)
        };

        let json = EventFields {
            message: message.unwrap_or_default(),
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            spans,
            fields,
        };

        writeln!(
            writer,
            "{}",
            serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string())
        )
    }
}

/// Write the verbosity level with the same coloring/alignment as the Full formatter.
fn write_colored_level(writer: &mut Writer<'_>, level: &Level) -> fmt::Result {
    if writer.has_ansi_escapes() {
        let paint = match *level {
            Level::TRACE => Paint::new("TRACE").magenta(),
            Level::DEBUG => Paint::new("DEBUG").blue(),
            Level::INFO => Paint::new(" INFO").green(),
            Level::WARN => Paint::new(" WARN").yellow(),
            Level::ERROR => Paint::new("ERROR").red(),
        };
        write!(writer, "{}", paint)
    } else {
        // Right-pad to width 5 like Full's non-ANSI mode
        match *level {
            Level::TRACE => write!(writer, "{:>5}", "TRACE"),
            Level::DEBUG => write!(writer, "{:>5}", "DEBUG"),
            Level::INFO => write!(writer, "{:>5}", " INFO"),
            Level::WARN => write!(writer, "{:>5}", " WARN"),
            Level::ERROR => write!(writer, "{:>5}", "ERROR"),
        }
    }
}

fn write_dimmed(writer: &mut Writer<'_>, s: impl fmt::Display) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new(s).dim())
    } else {
        write!(writer, "{}", s)
    }
}

fn write_bold(writer: &mut Writer<'_>, s: impl fmt::Display) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{}", Paint::new(s).bold())
    } else {
        write!(writer, "{}", s)
    }
}
