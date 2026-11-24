use gpui_kit::SharedString;
use threadbear_core::ja;

pub(crate) struct Faces {
    pub(crate) name: SharedString,
    pub(crate) shown: SharedString,
}

impl Faces {
    fn same(text: SharedString) -> Faces {
        Faces {
            name: text.clone(),
            shown: text,
        }
    }

    pub(crate) fn of_shared(text: SharedString) -> Faces {
        known(&text).unwrap_or_else(|| Faces::same(text))
    }
}

fn known(text: &str) -> Option<Faces> {
    if !ja::screen_is_japanese() {
        return None;
    }
    if let Some((en, ja_words)) = entry(text) {
        return Some(Faces {
            name: SharedString::new_static(en),
            shown: SharedString::new_static(if ja::on() { ja_words } else { en }),
        });
    }
    ja::made_shown(text).map(|shown| Faces {
        name: SharedString::from(text),
        shown: shown.into(),
    })
}

fn entry(text: &str) -> Option<(&'static str, &'static str)> {
    ja::entry_of(text, &ja::TABLE)
        .or_else(|| ja::entry_of(text, &crate::ja_table::TABLE))
        .copied()
}

pub(crate) trait IntoFaces {
    fn into_faces(self) -> Faces;
}

impl IntoFaces for Faces {
    fn into_faces(self) -> Faces {
        self
    }
}

impl IntoFaces for &str {
    fn into_faces(self) -> Faces {
        known(self).unwrap_or_else(|| Faces::same(self.into()))
    }
}

impl IntoFaces for &String {
    fn into_faces(self) -> Faces {
        self.as_str().into_faces()
    }
}

impl IntoFaces for String {
    fn into_faces(self) -> Faces {
        known(&self).unwrap_or_else(|| Faces::same(self.into()))
    }
}

impl IntoFaces for SharedString {
    fn into_faces(self) -> Faces {
        Faces::of_shared(self)
    }
}

impl IntoFaces for &SharedString {
    fn into_faces(self) -> Faces {
        Faces::of_shared(self.clone())
    }
}

impl<T: IntoFaces> IntoFaces for Both<T> {
    fn into_faces(self) -> Faces {
        let name = self.en.into_faces();
        match self.ja {
            Some(ja_words) => Faces {
                name: name.name,
                shown: ja_words.into_faces().shown,
            },
            None => name,
        }
    }
}

pub(crate) fn faces<T: IntoFaces>(make: impl Fn() -> T) -> Faces {
    if !ja::on() {
        return make().into_faces();
    }
    let name = ja::english(|| make().into_faces()).name;
    Faces {
        name,
        shown: make().into_faces().shown,
    }
}

#[derive(Clone)]
pub(crate) struct Both<T> {
    en: T,
    ja: Option<T>,
}

pub(crate) fn both<T>(make: impl Fn() -> T) -> Both<T> {
    if !ja::on() {
        return Both {
            en: make(),
            ja: None,
        };
    }
    Both {
        en: ja::english(&make),
        ja: Some(make()),
    }
}

impl<A, B> Both<(A, B)> {
    #[allow(
        dead_code,
        reason = "used only while a copied file has such a pair; gpui allows dead code outside the automation build"
    )]
    pub(crate) fn split_0(self) -> (Both<A>, B) {
        let (en, other_en) = self.en;
        match self.ja {
            Some((ja_words, other)) => (
                Both {
                    en,
                    ja: Some(ja_words),
                },
                other,
            ),
            None => (Both { en, ja: None }, other_en),
        }
    }

    pub(crate) fn split_1(self) -> (A, Both<B>) {
        let (other_en, en) = self.en;
        match self.ja {
            Some((other, ja_words)) => (
                other,
                Both {
                    en,
                    ja: Some(ja_words),
                },
            ),
            None => (other_en, Both { en, ja: None }),
        }
    }
}

pub(crate) fn keep<T: Into<SharedString> + AsRef<str>>(text: T) -> SharedString {
    if !ja::screen_is_japanese() {
        return text.into();
    }
    match entry(text.as_ref()) {
        Some((en, _)) => SharedString::new_static(en),
        None => text.into(),
    }
}

pub(crate) fn shown_str(text: &'static str) -> &'static str {
    if !ja::screen_is_japanese() {
        return text;
    }
    match entry(text) {
        Some((en, ja_words)) => {
            if ja::on() {
                ja_words
            } else {
                en
            }
        }
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(en: &str) -> &'static (&'static str, &'static str) {
        ja::TABLE
            .iter()
            .find(|(e, _)| *e == en)
            .expect("updates.txt has the row")
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
    fn a_tooltip_made_from_the_table_is_named_in_english_and_shown_in_japanese() {
        let _ja = Japanese::on();
        let updates = row("Updates");
        let made = faces(|| ja::t(updates));
        assert_eq!(pair(&made), ("Updates", "更新"));
        let formatted = faces(|| format!("{} / {}", ja::t(updates), "bruce"));
        assert_eq!(pair(&formatted), ("Updates / bruce", "更新 / bruce"));
    }

    #[test]
    fn a_person_s_words_that_read_like_the_table_are_shown_as_they_are() {
        let _ja = Japanese::on();
        let runner = "Up".to_owned() + "dates";
        assert_eq!(pair(&faces(|| runner.clone())), ("Updates", "Updates"));
        assert_eq!(
            pair(&SharedString::from(runner.as_str()).into_faces()),
            ("Updates", "Updates")
        );
        assert_eq!(
            pair(&keep(runner.as_str()).into_faces()),
            ("Updates", "Updates")
        );
        assert_eq!(shown_str("Updates"), "Updates");
    }

    #[test]
    fn a_table_literal_finds_its_faces_wherever_it_s_handed() {
        let _ja = Japanese::on();
        let updates = row("Updates");
        let handed: &'static str = ja::t(updates);
        assert_eq!(handed, "更新");
        assert_eq!(pair(&faces(|| handed)), ("Updates", "更新"));
        let kept = keep(handed);
        assert_eq!(kept.as_str(), "Updates");
        assert_eq!(pair(&Faces::of_shared(kept.clone())), ("Updates", "更新"));
        assert_eq!(
            pair(&faces(|| handed.to_owned())),
            ("更新", "更新"),
            "a table word turned into a String can't be told from a person's"
        );
        assert_eq!(shown_str(updates.0), "更新");
    }

    #[test]
    fn a_variable_made_twice_keeps_both_its_faces() {
        let _ja = Japanese::on();
        let updates = row("Updates");
        let tip = both(|| format!("{}!", ja::t(updates)));
        assert_eq!(pair(&faces(|| tip.clone())), ("Updates!", "更新!"));
        let (text, tip) = both(|| (ja::t(updates), ja::t(updates))).split_1();
        assert_eq!(text, "更新");
        assert_eq!(pair(&tip.into_faces()), ("Updates", "更新"));
    }

    #[test]
    fn in_english_both_faces_are_the_english() {
        let updates = row("Updates");
        assert_eq!(pair(&faces(|| ja::t(updates))), ("Updates", "Updates"));
        assert_eq!(
            pair(&keep(ja::t(updates)).into_faces()),
            ("Updates", "Updates")
        );
        assert_eq!(shown_str(updates.0), "Updates");
    }
}
