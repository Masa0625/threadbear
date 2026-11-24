// Settings › General › Appearance's Language row, which only this copy has
// (desktop/ja/README.md): Same as the Mac, English or 日本語. The table for
// settings_window.rs puts it under the Font row.
//
// The choice is desktop-settings.json's "language", beside "appearance". It
// goes through the app's own settings, which keep a field they don't know
// (settings.rs), so saving any other setting later keeps it. Picking one
// changes every window's words there and then. What's made once as the app
// starts, the Mac's menu bar, follows at the next launch, when
// desktop/ja/start.rs reads the same key.
use crate::a11y::A11yRole as _;
use crate::app::Threadbear;
use crate::settings::Saved;
use crate::tips::Tip as _;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::Sizable as _;
use gpui_kit::*;
use serde_json::Value;

/// desktop-settings.json's key, the one start.rs reads.
const KEY: &str = "language";

/// The row's tooltip: in English as its name, which the activity log and
/// the scripts know it by, and the words it shows on a Japanese screen.
const TIP: &str = "The language for Threadbear's buttons, menus and messages";
const TIP_JA: &str = "Threadbear のボタン・メニュー・お知らせの言語";

/// What can be picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    System,
    English,
    Japanese,
}

impl Language {
    const ALL: [Language; 3] = [Language::System, Language::English, Language::Japanese];

    /// As desktop-settings.json keeps it.
    fn name(self) -> &'static str {
        match self {
            Language::System => "system",
            Language::English => "en",
            Language::Japanese => "ja",
        }
    }

    /// Anything else is the system's, as start.rs reads it.
    fn from_name(name: &str) -> Language {
        match name.trim() {
            "en" => Language::English,
            "ja" => Language::Japanese,
            _ => Language::System,
        }
    }

    /// What the menu calls it. English and 日本語 are in their own words on
    /// either screen, so someone who can't read the other finds theirs. The
    /// system's is named for the machine, as Appearance's Same as the Mac is.
    fn title(self, japanese: bool) -> &'static str {
        match (self, japanese) {
            (Language::English, _) => "English",
            (Language::Japanese, _) => "日本語",
            (Language::System, false) if cfg!(target_os = "macos") => "Same as the Mac",
            (Language::System, false) if cfg!(target_os = "windows") => "Same as Windows",
            (Language::System, false) => "Same as the system",
            (Language::System, true) if cfg!(target_os = "macos") => "Mac に合わせる",
            (Language::System, true) if cfg!(target_os = "windows") => "Windows に合わせる",
            (Language::System, true) => "システムに合わせる",
        }
    }

    /// Whether the screen's words are Japanese with this picked. The
    /// system's is read as start.rs reads it as the app starts, and the
    /// scripted build stays English with it.
    fn japanese(self) -> bool {
        match self {
            Language::English => false,
            Language::Japanese => true,
            Language::System => {
                !cfg!(feature = "automation") && crate::ja_start::system_is_japanese()
            }
        }
    }
}

/// The language picked, from the settings file's fields.
fn chosen(saved: &Saved) -> Language {
    saved
        .extra
        .get(KEY)
        .and_then(Value::as_str)
        .map_or(Language::System, Language::from_name)
}

/// Puts the choice among the settings file's fields, which the app's next
/// save writes with the rest.
fn keep(saved: &mut Saved, choice: Language) {
    saved
        .extra
        .insert(KEY.to_owned(), Value::from(choice.name()));
}

/// Picks `choice`: kept in the settings file, and every window drawn again
/// in its words.
fn pick(app: &mut Threadbear, choice: Language, cx: &mut Context<Threadbear>) {
    keep(&mut app.settings.saved, choice);
    app.save_settings_soon(cx);
    threadbear_core::ja::set_japanese(choice.japanese());
    cx.refresh_windows();
    cx.notify();
}

/// The row's words, in the screen's language.
pub(crate) fn label() -> &'static str {
    if threadbear_core::ja::on() {
        "言語"
    } else {
        "Language"
    }
}

/// The row's control, the same shape as Font's: a button saying what's
/// picked, whose menu ticks it. `tab` is the Settings window's section, whose
/// title the window shows: a pick changes that too.
pub(crate) fn control(app: WeakEntity<Threadbear>, tab: &'static str, cx: &App) -> AnyElement {
    let japanese = threadbear_core::ja::on();
    let now = app
        .upgrade()
        .map_or(Language::System, |a| chosen(&a.read(cx).settings.saved));
    let menu = Button::new("settings-language")
        .label(now.title(japanese))
        .small()
        .secondary()
        .dropdown_caret(true)
        .dropdown_menu(move |menu: PopupMenu, _, _| {
            let mut menu = menu;
            for choice in Language::ALL {
                let app = app.clone();
                menu = menu.item(
                    PopupMenuItem::new(choice.title(japanese))
                        .checked(choice == now)
                        .on_click(move |_, window, cx| {
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |app, cx| pick(app, choice, cx));
                            }
                            window.set_window_title(crate::ja_tip::shown_str(tab));
                        }),
                );
            }
            menu
        });
    // The tooltip on a holder round the kit's button, as tipped_as does for
    // one (highlights.rs), with its name and its words apart.
    div()
        .id("settings-language-holder")
        .test_support()
        .a11y_role(Role::GenericContainer)
        .aria_label(TIP)
        .ja_tip(crate::ja_tip::Faces {
            name: SharedString::new_static(TIP),
            shown: SharedString::new_static(if japanese { TIP_JA } else { TIP }),
        })
        .flex_none()
        .child(menu)
        .into_any_element()
}

/// The Settings window's dump of it: what's picked, and the screen's
/// language now.
#[cfg(feature = "automation")]
pub(crate) fn dump(app: &WeakEntity<Threadbear>, cx: &App) -> Value {
    let picked = app
        .upgrade()
        .map_or(Language::System, |a| chosen(&a.read(cx).settings.saved));
    serde_json::json!({
        "picked": picked.name(),
        "screen": if threadbear_core::ja::on() { "ja" } else { "en" },
    })
}

#[cfg(test)]
mod tests {
    // Named one by one: gpui_kit's glob has a `test` of its own, which
    // would stand in for the built-in one.
    use super::{chosen, keep, Language, KEY};
    use crate::settings::{Saved, Settings};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    /// A settings file holding `text`, in a folder of the test's own.
    fn file(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tb-language-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(Settings::file_name());
        std::fs::write(&path, text).unwrap();
        path
    }

    fn written(path: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn each_choice_is_kept_by_its_name_and_anything_else_is_the_system_s() {
        for choice in Language::ALL {
            assert_eq!(Language::from_name(choice.name()), choice);
        }
        assert_eq!(Language::Japanese.name(), "ja");
        assert_eq!(Language::from_name("fr"), Language::System);
        let mut saved = Saved::default();
        assert_eq!(chosen(&saved), Language::System, "nothing picked yet");
        saved.extra.insert(KEY.into(), Value::from(3));
        assert_eq!(chosen(&saved), Language::System, "not a name");
        saved.extra.insert(KEY.into(), Value::from("en"));
        assert_eq!(chosen(&saved), Language::English);
    }

    #[test]
    fn saving_the_choice_keeps_the_other_settings() {
        let path = file("keeps", r#"{ "appearance": "dark", "accent": "plum" }"#);
        let mut s = Settings::load_from(path.clone());
        keep(&mut s.saved, Language::Japanese);
        s.save_checked().unwrap();
        let w = written(&path);
        assert_eq!(w["language"], "ja");
        assert_eq!(w["appearance"], "dark");
        assert_eq!(w["accent"], "plum");
    }

    #[test]
    fn the_choice_stays_when_another_setting_is_saved() {
        let path = file("stays", r#"{ "appearance": "light" }"#);
        let mut s = Settings::load_from(path.clone());
        keep(&mut s.saved, Language::English);
        s.save_checked().unwrap();
        // The app as it next starts, then an accent picked and saved.
        let mut s = Settings::load_from(path.clone());
        assert_eq!(chosen(&s.saved), Language::English);
        s.saved.accent = Some("rose".into());
        s.save_checked().unwrap();
        let w = written(&path);
        assert_eq!(w["language"], "en");
        assert_eq!(w["accent"], "rose");
        assert_eq!(w["appearance"], "light");
        assert_eq!(chosen(&Settings::load_from(path).saved), Language::English);
    }

    #[test]
    fn english_and_japanese_are_named_in_their_own_words_on_either_screen() {
        for japanese in [false, true] {
            assert_eq!(Language::English.title(japanese), "English");
            assert_eq!(Language::Japanese.title(japanese), "日本語");
        }
        let (en, ja) = (Language::System.title(false), Language::System.title(true));
        assert!(en.starts_with("Same as "), "{en}");
        assert!(ja.ends_with("に合わせる"), "{ja}");
    }
}

/// How the table for settings_window.rs hands that window's tooltips on
/// (desktop/ja/gpui/settings_window.txt): each named in English, as the
/// activity log and the scripts know it, and shown in Japanese.
#[cfg(test)]
mod settings_window_tips {
    use crate::ja_tip::{both, faces, keep, shown_str, Faces};
    use threadbear_core::ja;

    /// A row of gpui's table, by its English.
    fn row(en: &str) -> &'static (&'static str, &'static str) {
        crate::ja_table::TABLE
            .iter()
            .find(|(e, _)| *e == en)
            .expect("settings_window.txt has the row")
    }

    struct Japanese;
    impl Japanese {
        fn on() -> Japanese {
            ja::set_here(Some(true));
            Japanese
        }
    }
    impl Drop for Japanese {
        fn drop(&mut self) {
            ja::set_here(None);
        }
    }

    fn pair(f: &Faces) -> (&str, &str) {
        (f.name.as_str(), f.shown.as_str())
    }

    #[test]
    fn a_switch_s_tooltip_handed_on_as_a_string_is_named_in_english() {
        let _ja = Japanese::on();
        let (en, ja_words) = *row("Note what you click and open in this app");
        // `switch` hands switch_owned a String, through ja::made.
        let handed: String = ja::made(en, || shown_str(en).to_owned());
        assert_eq!(handed, en);
        // switch_owned's `let shown` is made twice, then its `.tip(shown)`.
        let shown = both(|| handed.clone());
        assert_eq!(pair(&faces(|| shown.clone())), (en, ja_words));
    }

    #[test]
    fn a_section_s_about_is_named_in_english_and_shown_in_japanese() {
        let _ja = Japanese::on();
        let (en, ja_words) =
            *row("The background program that runs your threads. It updates itself.");
        let about: &str = en;
        let ja_about = faces(|| about);
        assert_eq!(pair(&ja_about), (en, ja_words));
        // The ⓘ's tooltip, which tipped makes from the name.
        assert_eq!(pair(&Faces::of_shared(ja_about.name)), (en, ja_words));
    }

    #[test]
    fn a_sidebar_row_kept_for_tipped_is_named_in_english() {
        let _ja = Japanese::on();
        let (en, ja_words) = *row("Phone & browser");
        let kept = keep(en);
        assert_eq!(kept.as_str(), en);
        assert_eq!(pair(&Faces::of_shared(kept)), (en, ja_words));
        assert_eq!(shown_str(en), ja_words);
    }

    #[test]
    fn on_an_english_screen_every_face_is_the_english() {
        let (en, _) = *row("Note what you click and open in this app");
        let handed = ja::made(en, || shown_str(en).to_owned());
        assert_eq!(pair(&faces(|| both(|| handed.clone()))), (en, en));
        assert_eq!(shown_str(row("Phone & browser").0), "Phone & browser");
    }
}
