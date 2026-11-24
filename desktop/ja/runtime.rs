// 画面の文の言語(`threadbear_core::ja`)。desktop/ja/README.md。
//
// 既定は英語。日本語になるのは、app が立ち上がる時に `set_japanese(true)` を
// 呼んだ時だけ(desktop/ja/start.rs)。試験は app を立ち上げないので、いつも英語。
//
// 写し(desktop/ja/mirror.rs)が、表にある literal を `t("英語", "日本語")` に、
// `format!("英語", …)` を `ja_format!("日本語", "英語", …)` に差し替える。
// ツールチップの文は記録と自動操作の名前を兼ねるので英語のまま渡り、描く所で
// `draw` が訳す。
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

static JAPANESE: AtomicBool = AtomicBool::new(false);

/// 画面の文を日本語にするか。app の立ち上げで 1 回だけ呼ぶ。
pub fn set_japanese(on: bool) {
    JAPANESE.store(on, Ordering::Relaxed);
}

/// 今、日本語で描くか。
pub fn on() -> bool {
    JAPANESE.load(Ordering::Relaxed)
}

/// 表の 1 つの文。どちらも literal のままなので、`&'static str` は変わらない。
pub fn t(en: &'static str, ja: &'static str) -> &'static str {
    if on() {
        ja
    } else {
        en
    }
}

/// `format!` の日本語版。書式は英語と同じ引数を使う(`{name}`・`{0}`)。
#[macro_export]
macro_rules! ja_format {
    ($ja:literal, $en:literal $($rest:tt)*) => {
        if $crate::ja::on() {
            format!($ja $($rest)*)
        } else {
            format!($en $($rest)*)
        }
    };
}

/// 描く時に訳す(ツールチップ)。表の英語と同じ文か、`{…}` の所を埋めた文なら
/// 日本語に、どちらでもなければ行ごとに見て、それでも無ければそのまま。
pub fn draw<'a>(text: &'a str, entries: &[(&str, &str)]) -> Cow<'a, str> {
    draw_in(on(), text, entries)
}

fn draw_in<'a>(japanese: bool, text: &'a str, entries: &[(&str, &str)]) -> Cow<'a, str> {
    if !japanese {
        return Cow::Borrowed(text);
    }
    if let Some(ja) = one(text, entries) {
        return Cow::Owned(ja);
    }
    if text.contains('\n') {
        let lines: Vec<String> = text
            .split('\n')
            .map(|l| one(l, entries).unwrap_or_else(|| l.to_owned()))
            .collect();
        return Cow::Owned(lines.join("\n"));
    }
    Cow::Borrowed(text)
}

fn one(text: &str, entries: &[(&str, &str)]) -> Option<String> {
    if let Some((_, ja)) = entries.iter().find(|(en, _)| *en == text) {
        return Some(unescape_braces(ja));
    }
    entries
        .iter()
        .filter(|(en, _)| en.contains('{'))
        .find_map(|(en, ja)| fill(en, ja, text))
}

/// 書式の文の 1 つの部分: 文字か、埋める所(鍵は名前か位置)。
enum Piece {
    Text(String),
    Hole(String),
}

/// `format!` の書式を部分に分ける。`{}` は前から 0, 1, …、`{2}` は 2、`{name}` は name。
fn pieces(format: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut next = 0;
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let mut spec = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    spec.push(c);
                }
                let name = spec.split(':').next().unwrap_or("").trim().to_owned();
                let key = if name.is_empty() {
                    next += 1;
                    (next - 1).to_string()
                } else {
                    name
                };
                out.push(Piece::Text(std::mem::take(&mut text)));
                out.push(Piece::Hole(key));
            }
            _ => text.push(c),
        }
    }
    out.push(Piece::Text(text));
    out
}

/// `text` が英語の書式 `en` に合えば、埋めた所を日本語の書式 `ja` に入れる。
fn fill(en: &str, ja: &str, text: &str) -> Option<String> {
    let en = pieces(en);
    let mut got: Vec<(String, String)> = Vec::new();
    let mut rest = text;
    let mut i = 0;
    while i < en.len() {
        match &en[i] {
            Piece::Text(t) => rest = rest.strip_prefix(t.as_str())?,
            Piece::Hole(key) => {
                // 次の文字の部分まで。最後なら残り全部。
                let until = match en.get(i + 1) {
                    Some(Piece::Text(t)) if !t.is_empty() && i + 2 < en.len() => rest.find(t.as_str())?,
                    Some(Piece::Text(t)) if !t.is_empty() => rest.len().checked_sub(t.len()).filter(|&n| rest[n..] == **t)?,
                    _ => rest.len(),
                };
                got.push((key.clone(), rest[..until].to_owned()));
                rest = &rest[until..];
            }
        }
        i += 1;
    }
    if !rest.is_empty() || got.iter().any(|(_, v)| v.is_empty()) {
        return None;
    }
    let mut out = String::new();
    for p in pieces(ja) {
        match p {
            Piece::Text(t) => out.push_str(&t),
            Piece::Hole(key) => out.push_str(&got.iter().find(|(k, _)| *k == key)?.1),
        }
    }
    Some(out)
}

fn unescape_braces(s: &str) -> String {
    s.replace("{{", "{").replace("}}", "}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 言語は 1 つの global で、英語を見るほかの試験と同時に流れるので、ここでは
    // 切り替えない。日本語の側は draw_in で見る。
    #[test]
    fn english_until_the_app_says_otherwise() {
        assert!(!on());
        assert_eq!(t("Dismiss", "閉じる"), "Dismiss");
        assert_eq!(ja_format!("{} 件", "{} things", 3), "3 things");
        assert_eq!(draw("Copy link", &[("Copy link", "リンクをコピー")]), "Copy link");
    }

    #[test]
    fn a_tooltip_is_drawn_from_the_table() {
        let entries = [
            ("Copy link", "リンクをコピー"),
            ("In {}", "{} の中"),
            ("Open {title} on {0}", "{0} で {title} を開く"),
            ("Updates", "更新"),
        ];
        let ja = |text| draw_in(true, text, &entries).into_owned();
        assert_eq!(ja("Copy link"), "リンクをコピー");
        assert_eq!(ja("In ~/dev/me"), "~/dev/me の中");
        assert_eq!(ja("Open Inbox on bruce"), "bruce で Inbox を開く");
        assert_eq!(ja("Updates\nCopy link"), "更新\nリンクをコピー");
        assert_eq!(ja("Something else"), "Something else");
        assert_eq!(ja("In "), "In ");
    }
}
