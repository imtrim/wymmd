use std::io;

use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

pub struct StdioTracingWriter;

fn writes_to_stderr(level: &Level) -> bool {
    level <= &Level::WARN
}

impl<'a> MakeWriter<'a> for StdioTracingWriter {
    type Writer = Box<dyn io::Write>;

    fn make_writer(&'a self) -> Self::Writer {
        Box::new(io::stdout())
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        if writes_to_stderr(meta.level()) {
            Box::new(io::stderr())
        } else {
            Box::new(io::stdout())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warn_and_below_go_to_stderr() {
        assert!(writes_to_stderr(&Level::ERROR));
        assert!(writes_to_stderr(&Level::WARN));
        assert!(!writes_to_stderr(&Level::INFO));
        assert!(!writes_to_stderr(&Level::DEBUG));
        assert!(!writes_to_stderr(&Level::TRACE));
    }
}
