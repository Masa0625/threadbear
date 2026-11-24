use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};

static JAPANESE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static ENGLISH_DEPTH: Cell<u32> = const { Cell::new(0) };
    static THIS_THREAD_JAPANESE: Cell<Option<bool>> = const { Cell::new(None) };
    static MADE: RefCell<Vec<MadeAnswer>> = const { RefCell::new(Vec::new()) };
}

struct MadeAnswer {
    producer: &'static str,
    english: String,
    japanese: String,
}

include!(concat!(env!("OUT_DIR"), "/ja/table.rs"));

pub fn set_japanese(on: bool) {
    JAPANESE.store(on, Ordering::Relaxed);
}

/// The language for this thread alone, `None` to follow the app. For tests,
/// which share one process.
#[doc(hidden)]
pub fn set_here(japanese: Option<bool>) {
    THIS_THREAD_JAPANESE.with(|j| j.set(japanese));
}

/// Whether the words made now are Japanese.
pub fn on() -> bool {
    ENGLISH_DEPTH.with(Cell::get) == 0 && screen_is_japanese()
}

/// Whether the screen is Japanese, even inside `english` (where the words
/// made are English, but table words handed in from outside may not be).
pub fn screen_is_japanese() -> bool {
    THIS_THREAD_JAPANESE
        .with(Cell::get)
        .unwrap_or_else(|| JAPANESE.load(Ordering::Relaxed))
}

/// `make`'s answer with every table word in it English.
pub fn english<T>(make: impl FnOnce() -> T) -> T {
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            ENGLISH_DEPTH.with(|e| e.set(e.get().saturating_sub(1)));
        }
    }
    ENGLISH_DEPTH.with(|e| e.set(e.get() + 1));
    let _leave = Leave;
    make()
}

/// One table entry, as the mirror writes it: `t(&TABLE[i])`. The answer is
/// the table's own `&'static str`, so a tooltip sink can tell it from a
/// person's words by its address (`entry_of`).
pub fn t(entry: &'static (&'static str, &'static str)) -> &'static str {
    if on() {
        entry.1
    } else {
        entry.0
    }
}

/// The entry whose English or Japanese `text` is, by address: the table's
/// own literal, never a person's words that happen to read the same.
pub fn entry_of(
    text: &str,
    table: &'static [(&'static str, &'static str)],
) -> Option<&'static (&'static str, &'static str)> {
    let same = |s: &str| s.as_ptr() == text.as_ptr() && s.len() == text.len();
    table.iter().find(|(en, ja)| same(en) || same(ja))
}

/// A function whose whole answer becomes a tooltip by way of Strings and
/// fields (the updates icon's, through the rail's button): made in English,
/// which is what it answers, and again for the words it shows, which a
/// tooltip finds by that exact English (`made_shown`). The mirror wraps the
/// body of a function a table names with `made:`.
pub fn made(producer: &'static str, make: impl Fn() -> String) -> String {
    if !on() {
        return make();
    }
    let en = english(&make);
    let ja = make();
    MADE.with(|m| {
        let mut m = m.borrow_mut();
        m.retain(|answer| answer.producer != producer);
        if ja != en {
            m.push(MadeAnswer {
                producer,
                english: en.clone(),
                japanese: ja,
            });
        }
    });
    en
}

/// The words to show for a tooltip that's the latest answer of a `made`
/// producer, exactly. Only those answers are here, so a person's words
/// that read like a table entry (or like one line of the answer) aren't.
pub fn made_shown(name: &str) -> Option<String> {
    if !on() {
        return None;
    }
    MADE.with(|m| {
        m.borrow()
            .iter()
            .find(|answer| answer.english == name)
            .map(|answer| answer.japanese.clone())
    })
}

/// `format!` in Japanese or English, the same arguments for either
/// (`{name}`, `{0}`). A Japanese format that leaves out one of English's
/// positional arguments takes it as `{0:.0}`, which writes nothing.
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

#[cfg(test)]
mod tests {
    use super::*;

    static ENTRY: (&str, &str) = ("Dismiss", "閉じる");

    fn typed() -> String {
        "Dis".to_owned() + "miss"
    }

    #[test]
    fn english_until_the_app_says_otherwise() {
        assert!(!on());
        assert_eq!(t(&ENTRY), "Dismiss");
        assert_eq!(ja_format!("{} 件", "{} things", 3), "3 things");
    }

    #[test]
    fn english_holds_inside_japanese_and_comes_back() {
        set_here(Some(true));
        assert_eq!(t(&ENTRY), "閉じる");
        let inner = english(|| (t(&ENTRY), english(|| t(&ENTRY)), t(&ENTRY)));
        assert_eq!(inner, ("Dismiss", "Dismiss", "Dismiss"));
        assert_eq!(t(&ENTRY), "閉じる");
        assert_eq!(ja_format!("{} 件", "{} things", 3), "3 件");
        set_here(None);
    }

    #[test]
    fn an_entry_is_found_by_address_not_by_its_words() {
        static TWO: [(&str, &str); 1] = [("Dismiss", "閉じる")];
        set_here(Some(true));
        assert!(entry_of(t(&TWO[0]), &TWO).is_some());
        assert!(english(|| entry_of(t(&TWO[0]), &TWO)).is_some());
        assert_eq!(typed(), "Dismiss");
        assert!(entry_of(&typed(), &TWO).is_none());
        assert!(entry_of("閉じる".to_owned().as_str(), &TWO).is_none());
        set_here(None);
    }

    #[test]
    fn a_producer_answers_english_and_keeps_its_japanese_for_the_tooltip() {
        let make = || format!("{}\n{}", t(&ENTRY), "Runner words");
        assert_eq!(made("p", make), "Dismiss\nRunner words");
        assert_eq!(made_shown("Dismiss\nRunner words"), None);
        set_here(Some(true));
        assert_eq!(made("p", make), "Dismiss\nRunner words");
        assert_eq!(
            made_shown("Dismiss\nRunner words").as_deref(),
            Some("閉じる\nRunner words")
        );
        assert_eq!(made_shown("Dismiss"), None);
        assert_eq!(made_shown("Runner words"), None);
        assert_eq!(made("p", || "Runner words".to_owned()), "Runner words");
        assert_eq!(made_shown("Dismiss\nRunner words"), None);
        set_here(None);
    }
}
