import re

with open('src/systems/tracing.rs', 'r') as f:
    content = f.read()

search = """impl Subscriber for FmtSubscriber {
    fn event(&self, event: &Event, span_context: &[Span]) {
        // Build the span path (e.g., "[server -> request_handler]")
        let span_path = if span_context.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = span_context.iter().map(|s| s.name).collect();
            format!("[{}] ", names.join(" -> "))
        };

        // Build span fields (e.g., "{req_id=123, user_id=456}")
        let mut all_span_fields = Vec::new();
        for span in span_context {
            for field in &span.fields {
                all_span_fields.push(format!("{}={}", field.key, field.value));
            }
        }
        let span_fields_str = if all_span_fields.is_empty() {
            String::new()
        } else {
            format!(" {{{}}}", all_span_fields.join(", "))
        };

        // Build event fields
        let event_fields_str = if event.fields.is_empty() {
            String::new()
        } else {
            let fields: Vec<String> = event
                .fields
                .iter()
                .map(|f| format!("{}={}", f.key, f.value))
                .collect();
            format!(" ({})", fields.join(", "))
        };

        println!(
            "{} {}{}{}{} {}",
            event.level,
            span_path,
            event.message,
            event_fields_str,
            span_fields_str,
            event.timestamp
        );
    }
}"""

replace = """impl Subscriber for FmtSubscriber {
    fn event(&self, event: &Event, span_context: &[Span]) {
        use std::fmt::Write;

        // ⚡ BOLT OPTIMIZATION: Avoid intermediate `Vec` and `String` allocations
        // by formatting directly into a pre-allocated `String` buffer.
        let mut output = String::with_capacity(256);

        let _ = write!(output, "{} ", event.level);

        if !span_context.is_empty() {
            output.push('[');
            for (i, span) in span_context.iter().enumerate() {
                if i > 0 {
                    output.push_str(" -> ");
                }
                output.push_str(span.name);
            }
            output.push_str("] ");
        }

        output.push_str(&event.message);

        if !event.fields.is_empty() {
            output.push_str(" (");
            for (i, field) in event.fields.iter().enumerate() {
                if i > 0 {
                    output.push_str(", ");
                }
                let _ = write!(output, "{}={}", field.key, field.value);
            }
            output.push(')');
        }

        let mut first_span_field = true;
        for span in span_context {
            for field in &span.fields {
                if first_span_field {
                    output.push_str(" {");
                    first_span_field = false;
                } else {
                    output.push_str(", ");
                }
                let _ = write!(output, "{}={}", field.key, field.value);
            }
        }
        if !first_span_field {
            output.push('}');
        }

        let _ = write!(output, " {}", event.timestamp);

        println!("{}", output);
    }
}"""

if search in content:
    content = content.replace(search, replace)
    with open('src/systems/tracing.rs', 'w') as f:
        f.write(content)
    print("Replaced successfully!")
else:
    print("Search string not found.")
