const LOG_LEVEL_ENV: &str = "SL_LOG";
const APP_LOG_TARGET: &str = "sealantern_connect";
const LOG_FILE_NAME: &str = "sealantern-connect";
pub(crate) const OPENFRP_LOG_TARGET: &str = "frp::openfrp";
pub(crate) const SAKURAFRP_LOG_TARGET: &str = "frp::sakurafrp";
const MAX_LOG_FILE_SIZE: u128 = 10_000_000;
const MAX_ROTATED_FILES: usize = 10;

fn is_app_log(metadata: &log::Metadata<'_>) -> bool {
    metadata.target() == APP_LOG_TARGET || metadata.target().starts_with("sealantern_connect::")
}

fn is_openfrp_log(metadata: &log::Metadata<'_>) -> bool {
    metadata.target() == OPENFRP_LOG_TARGET
}

fn is_sakurafrp_log(metadata: &log::Metadata<'_>) -> bool {
    metadata.target() == SAKURAFRP_LOG_TARGET
}

pub(crate) fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let file = tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
        file_name: Some(LOG_FILE_NAME.to_owned()),
    })
    .filter(is_app_log)
    .format(|out, _message, record| {
        let now = tauri_plugin_log::TimezoneStrategy::UseLocal.get_now();
        out.finish(format_args!(
            "[{:04}-{:02}-{:02}][{:02}:{:02}:{:02}][{}][{}] {}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            record.level(),
            record.target(),
            record.args(),
        ));
    });
    let openfrp_file = tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
        file_name: Some("openfrp".to_owned()),
    })
    .filter(is_openfrp_log)
    .format(|out, _message, record| {
        let now = tauri_plugin_log::TimezoneStrategy::UseLocal.get_now();
        out.finish(format_args!(
            "[{:04}-{:02}-{:02}][{:02}:{:02}:{:02}] {}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            record.args(),
        ));
    });
    let sakurafrp_file = tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
        file_name: Some("sakurafrp".to_owned()),
    })
    .filter(is_sakurafrp_log)
    .format(|out, _message, record| {
        let now = tauri_plugin_log::TimezoneStrategy::UseLocal.get_now();
        out.finish(format_args!(
            "[{:04}-{:02}-{:02}][{:02}:{:02}:{:02}] {}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            record.args(),
        ));
    });
    #[cfg(debug_assertions)]
    let targets = vec![
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout)
            .filter(is_app_log)
            .format(|out, _message, record| {
                let (label, color) = match record.level() {
                    log::Level::Error => ("ERROR", 31),
                    log::Level::Warn => ("WARN", 33),
                    log::Level::Info => ("INFO", 32),
                    log::Level::Debug => ("DEBUG", 34),
                    log::Level::Trace => ("TRACE", 90),
                };
                out.finish(format_args!(
                    "\x1b[{color}m[{label}]\x1b[0m {}",
                    record.args()
                ));
            }),
        file,
        openfrp_file,
        sakurafrp_file,
    ];
    #[cfg(not(debug_assertions))]
    let targets = vec![file, openfrp_file, sakurafrp_file];

    tauri_plugin_log::Builder::new()
        .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
        .clear_format()
        .clear_targets()
        .targets(targets)
        .level(level())
        .level_for("frp", log::LevelFilter::Info)
        .max_file_size(MAX_LOG_FILE_SIZE)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(
            MAX_ROTATED_FILES,
        ))
        .build()
}

fn level() -> log::LevelFilter {
    std::env::var(LOG_LEVEL_ENV)
        .map(|value| parse_level(&value))
        .unwrap_or(log::LevelFilter::Info)
}

fn parse_level(value: &str) -> log::LevelFilter {
    match value.trim().to_ascii_lowercase().as_str() {
        "debug" => log::LevelFilter::Debug,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_levels() {
        assert_eq!(parse_level("debug"), log::LevelFilter::Debug);
        assert_eq!(parse_level(" INFO "), log::LevelFilter::Info);
        assert_eq!(parse_level("Warn"), log::LevelFilter::Warn);
        assert_eq!(parse_level("ERROR"), log::LevelFilter::Error);
        assert_eq!(parse_level("trace"), log::LevelFilter::Info);
    }
}
