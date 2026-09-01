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
use crate::style;
use crate::style::Paint;

/// A span's own fields, kept as they were recorded when it opened rather than
/// re-recorded on every event inside it.
struct SpanFields(Line);

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

        span.extensions_mut().insert(SpanFields(line));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();

        let mut line = Line::default();
        event.record(&mut line);

        // Spans, outermost first, so a line reads as "what we were doing" then
        // "what happened": `quote{symbol=SPY} → GET url=…`.
        let mut scopes = Vec::new();

        if let Some(spans) = ctx.event_scope(event) {
            for span in spans.from_root() {
                let fields = span
                    .extensions()
                    .get::<SpanFields>()
                    .map(|f| f.0.clone())
                    .unwrap_or_default();

                scopes.push((span.name(), fields));
            }
        }

        // Colour is per sink, but two sinks wanting the same thing render it
        // once: the console and the file do not pay for each other.
        let mut plain = None;
        let mut coloured = None;

        for sink in &self.sinks {
            let paint = sink.paint();

            let cached = match paint {
                Paint::Off => &mut plain,
                Paint::Ansi => &mut coloured,
            };

            let rendered = cached.get_or_insert_with(|| render(meta, &scopes, &line, paint));

            sink.write(rendered);
        }
    }
}

fn render(
    meta: &tracing::Metadata<'_>,
    scopes: &[(&str, Line)],
    line: &Line,
    paint: Paint,
) -> String {
    let mut out = String::new();

    paint.write(&mut out, style::DIM, &crate::clock::now());
    out.push(' ');
    paint.write(&mut out, style::DIM, crate::SHA);
    out.push(' ');

    paint.write(&mut out, style::level(meta.level()), meta.level().as_str());

    // `as_str` is the level without padding, and the column has to stay put.
    for _ in meta.level().as_str().len()..5 {
        out.push(' ');
    }

    // The module the line came from, which for a line logged on somebody's
    // behalf is the caller's rather than ours. See `Line::module`.
    out.push(' ');
    paint.write(
        &mut out,
        style::TARGET,
        line.module().unwrap_or_else(|| meta.target()),
    );

    for (name, fields) in scopes {
        out.push(' ');
        paint.write(&mut out, style::BOLD, name);

        if !fields.is_empty() {
            paint.write(&mut out, style::DIM, "{");
            out.push_str(&fields.render(paint));
            paint.write(&mut out, style::DIM, "}");
        }
    }

    if !line.is_empty() {
        out.push(' ');
        out.push_str(&line.render(paint));
    }

    out
}
