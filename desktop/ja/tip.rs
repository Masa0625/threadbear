// A tooltip's two faces in the Japanese copy (desktop/ja/README.md): its
// name, which the activity log and the scripts know a control by and which
// stays English, and the words it shows.
//
// The words are Japanese only when the code made them from a table, which
// is told by how they arrive, never by how they read: a person's words or a
// runner's that read like a table entry are shown as they are.
// - `faces` makes a tooltip twice, in English for its name and again for its
//   words. The mirror puts it in every `.tip(…)` and `tipped!` of a copied
//   file, and `both` in the `let` of a variable that becomes one.
// - A table's own `&'static str` (`t(&TABLE[i])`) is known by its address,
//   so one handed on through a parameter, or kept as a SharedString by
//   `keep`, finds its English name and its Japanese wherever it ends up.
// - A `made` function's answer (the updates icon's) is known by being its
//   latest answer, exactly (`threadbear_core::ja::made_shown`).
use gpui_kit::SharedString;
use threadbear_core::ja;

/// A tooltip: the name a click on it is logged and found by, and its words.
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

    /// A tooltip that came as a SharedString, as most do: a table's literal
    /// kept by `keep`, or a `made` answer, gets its Japanese; anything else
    /// is shown as it is.
    pub(crate) fn of_shared(text: SharedString) -> Faces {
        known(&text).unwrap_or_else(|| Faces::same(text))
    }
}

/// The faces of words the code made from a table, told by their address or
/// as a `made` answer; None for anything else.
fn known(text: &str) -> Option<Faces> {
    // In English every word is already its own name: nothing to look up.
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

/// The table entry `text` is one of the literals of, by its address.
fn entry(text: &str) -> Option<(&'static str, &'static str)> {
    ja::entry_of(text, &ja::TABLE)
        .or_else(|| ja::entry_of(text, &crate::ja_table::TABLE))
        .copied()
}

/// What a tooltip can be made from.
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

/// A tooltip made twice: `make` in English for its name, and again for its
/// words while the screen is Japanese. Anything in it a person or a runner
/// wrote comes out the same both times, so it's shown as it is.
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

/// A variable's value made twice, as `faces` does, for a variable that
/// becomes a tooltip later (the mirror wraps its `let`).
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
    /// `let (tip, text) = …`: the first is the tooltip, the second the
    /// words as made for the screen.
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

    /// `let (text, tip) = …`: the second is the tooltip.
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

/// A tooltip handed on as a SharedString (a PushButton's, `tipped_as`'s): a
/// table's literal keeps its address, as the English name, so the tooltip
/// it ends up in still finds the Japanese.
pub(crate) fn keep<T: Into<SharedString> + AsRef<str>>(text: T) -> SharedString {
    if !ja::screen_is_japanese() {
        return text.into();
    }
    match entry(text.as_ref()) {
        Some((en, _)) => SharedString::new_static(en),
        None => text.into(),
    }
}

/// The words to draw for a table literal that's also an id or a screen
/// reader's name, so stayed English where it was written (`tip` lines).
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

    /// A row of core's table, by its English (desktop/ja/core/updates.txt).
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
        // A runner's message, a thread's title: the same words as an entry.
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
        // Made outside `faces` (a parameter's value), in Japanese: the name
        // goes back to English.
        let handed: &'static str = ja::t(updates);
        assert_eq!(handed, "更新");
        assert_eq!(pair(&faces(|| handed)), ("Updates", "更新"));
        // Kept as a SharedString by a PushButton, then a tooltip.
        let kept = keep(handed);
        assert_eq!(kept.as_str(), "Updates");
        assert_eq!(pair(&Faces::of_shared(kept.clone())), ("Updates", "更新"));
        // Copied into a String on the way, it's someone's words again.
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
