use threadbear_client::config::RunnerConfig;

pub(crate) fn start(automation: bool) {
    let asked = std::env::var("THREADBEAR_LANG").ok();
    let saved = if automation { None } else { saved_choice() };
    threadbear_core::ja::set_japanese(japanese(
        asked.as_deref().or(saved.as_deref()),
        automation,
        system_is_japanese,
    ));
}

fn japanese(choice: Option<&str>, automation: bool, system: fn() -> bool) -> bool {
    match choice.map(str::trim) {
        Some("ja") => true,
        Some("en") => false,
        _ => !automation && system(),
    }
}

fn saved_choice() -> Option<String> {
    let path = crate::settings::Settings::path_in(&RunnerConfig::from_env().home);
    language_in(&std::fs::read(path).ok()?)
}

fn language_in(data: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    value.get("language")?.as_str().map(str::to_owned)
}

#[cfg(target_os = "macos")]
pub(crate) fn system_is_japanese() -> bool {
    let languages = objc2_foundation::NSLocale::preferredLanguages();
    languages
        .firstObject()
        .is_some_and(|first| first.to_string().starts_with("ja"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_is_japanese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.starts_with("ja"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_comes_from_the_settings_beside_the_appearance() {
        let saved = br#"{ "appearance": "dark", "language": "ja" }"#;
        assert_eq!(language_in(saved).as_deref(), Some("ja"));
        assert_eq!(language_in(br#"{ "appearance": "dark" }"#), None);
        assert_eq!(language_in(b"not json"), None);
    }

    #[test]
    fn system_follows_macos_and_the_scripted_build_stays_english() {
        let (yes, no): (fn() -> bool, fn() -> bool) = (|| true, || false);
        assert!(japanese(Some("ja"), false, no));
        assert!(!japanese(Some("en"), false, yes));
        assert!(japanese(Some("system"), false, yes));
        assert!(japanese(None, false, yes));
        assert!(!japanese(None, false, no));
        assert!(!japanese(Some("system"), true, yes));
        assert!(japanese(Some("ja"), true, no));
    }
}
