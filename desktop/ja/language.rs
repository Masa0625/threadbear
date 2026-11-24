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

const KEY: &str = "language";

const TIP: &str = "The language for Threadbear's buttons, menus and messages";
const TIP_JA: &str = "Threadbear のボタン・メニュー・お知らせの言語";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    System,
    English,
    Japanese,
}

impl Language {
    const ALL: [Language; 3] = [Language::System, Language::English, Language::Japanese];

    fn setting_value(self) -> &'static str {
        match self {
            Language::System => "system",
            Language::English => "en",
            Language::Japanese => "ja",
        }
    }

    fn from_setting_value(value: &str) -> Language {
        match value.trim() {
            "en" => Language::English,
            "ja" => Language::Japanese,
            _ => Language::System,
        }
    }

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

fn chosen(saved: &Saved) -> Language {
    saved
        .extra
        .get(KEY)
        .and_then(Value::as_str)
        .map_or(Language::System, Language::from_setting_value)
}

fn set_in_settings(saved: &mut Saved, choice: Language) {
    saved
        .extra
        .insert(KEY.to_owned(), Value::from(choice.setting_value()));
}

fn pick(app: &mut Threadbear, choice: Language, cx: &mut Context<Threadbear>) {
    set_in_settings(&mut app.settings.saved, choice);
    app.save_settings_soon(cx);
    threadbear_core::ja::set_japanese(choice.japanese());
    cx.refresh_windows();
    cx.notify();
}

pub(crate) fn label() -> &'static str {
    if threadbear_core::ja::on() {
        "言語"
    } else {
        "Language"
    }
}

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

#[cfg(feature = "automation")]
pub(crate) fn dump(app: &WeakEntity<Threadbear>, cx: &App) -> Value {
    let picked = app
        .upgrade()
        .map_or(Language::System, |a| chosen(&a.read(cx).settings.saved));
    serde_json::json!({
        "picked": picked.setting_value(),
        "screen": if threadbear_core::ja::on() { "ja" } else { "en" },
    })
}

#[cfg(test)]
mod tests {
    // Named one by one: gpui_kit's glob has a `test` of its own, which
    // would stand in for the built-in one.
    use super::{chosen, set_in_settings, Language, KEY};
    use crate::settings::{Saved, Settings};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

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
    fn each_choice_is_saved_as_its_setting_value_and_anything_else_is_the_system_s() {
        for choice in Language::ALL {
            assert_eq!(Language::from_setting_value(choice.setting_value()), choice);
        }
        assert_eq!(Language::Japanese.setting_value(), "ja");
        assert_eq!(Language::from_setting_value("fr"), Language::System);
        let mut saved = Saved::default();
        assert_eq!(chosen(&saved), Language::System, "nothing picked yet");
        saved.extra.insert(KEY.into(), Value::from(3));
        assert_eq!(chosen(&saved), Language::System, "not a setting value");
        saved.extra.insert(KEY.into(), Value::from("en"));
        assert_eq!(chosen(&saved), Language::English);
    }

    #[test]
    fn saving_the_choice_keeps_the_other_settings() {
        let path = file("keeps", r#"{ "appearance": "dark", "accent": "plum" }"#);
        let mut s = Settings::load_from(path.clone());
        set_in_settings(&mut s.saved, Language::Japanese);
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
        set_in_settings(&mut s.saved, Language::English);
        s.save_checked().unwrap();
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

#[cfg(test)]
mod settings_window_tips {
    use crate::ja_tip::{both, faces, shown_str, to_table_english, Faces};
    use threadbear_core::ja;

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
        let handed: String = ja::made(en, || shown_str(en).to_owned());
        assert_eq!(handed, en);
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
        assert_eq!(pair(&Faces::of_shared(ja_about.name)), (en, ja_words));
    }

    #[test]
    fn a_sidebar_row_kept_for_tipped_is_named_in_english() {
        let _ja = Japanese::on();
        let (en, ja_words) = *row("Phone & browser");
        let kept = to_table_english(en);
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
