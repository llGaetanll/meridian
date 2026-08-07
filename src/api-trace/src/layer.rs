//! The `tracing` layer that renders events and hands them to the sinks.

use tracing::Event;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::registry::LookupSpan;

use crate::line::Line;
use crate::sink::Sink;

/// A span's own fields, rendered once when it opens rather than on every event
/// inside it.
struct SpanFields(String);

pub struct OneLine {
    sinks: Vec<Box<dyn Sink>>,
}

impl OneLine {
    pub fn new(sinks: Vec<Box<dyn Sink>>) -> Self {
        Self { sinks }
    }
}

impl<S> Layer<S> for OneLine
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        let mut line = Line::default();
        attrs.record(&mut line);

        span.extensions_mut().insert(SpanFields(line.finish()));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();

        let mut line = Line::default();
        event.record(&mut line);

        // Spans, outermost first, so a line reads as "what we were doing" then
        // "what happened": `quote{symbol=SPY} → GET url=…`.
        let mut scope = String::new();

        if let Some(spans) = ctx.event_scope(event) {
            for span in spans.from_root() {
                let fields = span
                    .extensions()
                    .get::<SpanFields>()
                    .map(|f| f.0.clone())
                    .unwrap_or_default();

                scope.push_str(span.name());

                if !fields.is_empty() {
                    scope.push('{');
                    scope.push_str(&fields);
                    scope.push('}');
                }

                scope.push(' ');
            }
        }

        let rendered = format!(
            "{} {} {:<5} {} {}{}",
            crate::clock::now(),
            crate::SHA,
            meta.level(),
            meta.target(),
            scope,
            line.finish(),
        );

        for sink in &self.sinks {
            sink.write(&rendered);
        }
    }
}
