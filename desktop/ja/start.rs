// app の立ち上げで、画面の文の言語を決める(desktop/ja/README.md)。写しの
// `fn main` の始めから呼ばれる(desktop/ja/mirror.rs)。
//
// 値は「system(システムに合わせる)/ en / ja」の 3 つ。先に環境変数
// THREADBEAR_LANG、無ければデータのフォルダの desktop-language.json の
// `language`、どちらも無ければ system。system は macOS の優先言語の一番上が
// `ja` で始まれば日本語。自動操作の build(specs)は、環境変数で頼まれない限り英語。
use threadbear_client::config::RunnerConfig;

pub(crate) fn start(automation: bool) {
    let asked = std::env::var("THREADBEAR_LANG").ok();
    let saved = if automation { None } else { saved_choice() };
    let japanese = match asked.as_deref().or(saved.as_deref()).map(str::trim) {
        Some("ja") => true,
        Some("en") => false,
        _ => !automation && system_is_japanese(),
    };
    threadbear_core::ja::set_japanese(japanese);
}

/// desktop-language.json の `{"language": "ja"}`。
fn saved_choice() -> Option<String> {
    let path = RunnerConfig::from_env().home.join("desktop-language.json");
    let data = std::fs::read(path).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&data).ok()?;
    value.get("language")?.as_str().map(str::to_owned)
}

#[cfg(target_os = "macos")]
fn system_is_japanese() -> bool {
    let languages = objc2_foundation::NSLocale::preferredLanguages();
    languages
        .firstObject()
        .is_some_and(|first| first.to_string().starts_with("ja"))
}

#[cfg(not(target_os = "macos"))]
fn system_is_japanese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.starts_with("ja"))
}
