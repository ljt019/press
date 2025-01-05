pub fn setup_logger(config: &super::config::Config) {
    env_logger::Builder::from_default_env()
        .filter_level(match config.log_level.as_str() {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Off,
        })
        .init();
}
