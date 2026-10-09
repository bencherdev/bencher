pub fn discard() -> slog::Logger {
    slog::Logger::root(slog::Discard, slog::o!())
}
