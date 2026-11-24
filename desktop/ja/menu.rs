// GPUI finds the Window menu by its English title, so the menus are made
// in English and retitled by place afterwards. macOS adds items of its own
// to some menus (Start Dictation and Emoji & Symbols in Edit, Enter Full
// Screen in View, the windows in Window), and one it puts among ours moves
// the rest down, so an item is retitled only while its title is still the
// English made for its place.

pub(crate) enum Place<'a> {
    Titled {
        en: &'a str,
        ja: &'a str,
        items: Vec<Place<'a>>,
    },
    Kept,
}

pub(crate) trait Menu: Sized {
    fn item(&self, at: usize) -> Option<(String, Option<Self>)>;
    fn set_title(&self, at: usize, title: &str);
}

pub(crate) fn retitle<M: Menu>(menu: &M, places: &[Place<'_>]) {
    for (at, place) in places.iter().enumerate() {
        let Place::Titled { en, ja, items } = place else {
            continue;
        };
        let Some((now, submenu)) = menu.item(at) else {
            return;
        };
        if now != *en {
            continue;
        }
        if ja != en {
            menu.set_title(at, ja);
        }
        if let Some(submenu) = submenu {
            retitle(&submenu, items);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct Fake(Rc<RefCell<Vec<(String, Option<Fake>)>>>);

    impl Fake {
        fn of(items: Vec<(&str, Option<Fake>)>) -> Fake {
            Fake(Rc::new(RefCell::new(
                items.into_iter().map(|(t, s)| (t.to_owned(), s)).collect(),
            )))
        }

        fn titles(&self) -> Vec<String> {
            self.0.borrow().iter().map(|(t, _)| t.clone()).collect()
        }
    }

    impl Menu for Fake {
        fn item(&self, at: usize) -> Option<(String, Option<Fake>)> {
            self.0.borrow().get(at).cloned()
        }

        fn set_title(&self, at: usize, title: &str) {
            if let Some(item) = self.0.borrow_mut().get_mut(at) {
                item.0 = title.to_owned();
            }
        }
    }

    fn item<'a>(en: &'a str, ja: &'a str) -> Place<'a> {
        Place::Titled {
            en,
            ja,
            items: Vec::new(),
        }
    }

    fn menu<'a>(en: &'a str, items: Vec<Place<'a>>) -> Place<'a> {
        Place::Titled { en, ja: en, items }
    }

    fn thread_places() -> Vec<Place<'static>> {
        vec![
            item("Archive", "アーカイブ"),
            Place::Titled {
                en: "Hibernate",
                ja: "休止",
                items: vec![
                    item("1 hour", "1 時間"),
                    item("Pick a time…", "時刻を選ぶ…"),
                ],
            },
            Place::Kept,
            item("Stop", "止める"),
        ]
    }

    fn hibernate() -> Fake {
        Fake::of(vec![("1 hour", None), ("Pick a time…", None)])
    }

    #[test]
    fn each_item_takes_its_japanese_in_the_place_it_was_made() {
        let sub = hibernate();
        let thread = Fake::of(vec![
            ("Archive", None),
            ("Hibernate", Some(sub.clone())),
            ("", None),
            ("Stop", None),
        ]);
        retitle(&thread, &thread_places());
        assert_eq!(thread.titles(), ["アーカイブ", "休止", "", "止める"]);
        assert_eq!(sub.titles(), ["1 時間", "時刻を選ぶ…"]);
    }

    #[test]
    fn an_item_macos_puts_among_ours_moves_no_title_onto_another_command() {
        let sub = hibernate();
        let thread = Fake::of(vec![
            ("Archive", None),
            ("Start Dictation…", None),
            ("Hibernate", Some(sub.clone())),
            ("", None),
            ("Stop", None),
        ]);
        retitle(&thread, &thread_places());
        assert_eq!(
            thread.titles(),
            ["アーカイブ", "Start Dictation…", "Hibernate", "", "Stop"],
            "only the item still in its own place is retitled"
        );
        assert_eq!(
            sub.titles(),
            ["1 hour", "Pick a time…"],
            "a submenu whose item moved is left as it is"
        );
    }

    #[test]
    fn items_macos_adds_after_ours_and_ours_it_left_out_are_left_alone() {
        let window = Fake::of(vec![
            ("Minimize", None),
            ("Zoom", None),
            ("Bring All to Front", None),
            ("A window", None),
        ]);
        retitle(
            &window,
            &[
                item("Minimize", "しまう"),
                item("Zoom", "拡大/縮小"),
                item("Bring All to Front", "すべてを手前に移動"),
            ],
        );
        assert_eq!(
            window.titles(),
            ["しまう", "拡大/縮小", "すべてを手前に移動", "A window"]
        );
        let short = Fake::of(vec![("Minimize", None)]);
        retitle(
            &short,
            &[item("Minimize", "しまう"), item("Zoom", "拡大/縮小")],
        );
        assert_eq!(short.titles(), ["しまう"]);
    }

    #[test]
    fn a_menu_bar_whose_menu_moved_keeps_that_menu_english() {
        let file = Fake::of(vec![("New Thread", None)]);
        let thread = Fake::of(vec![("Archive", None)]);
        let bar = Fake::of(vec![
            ("Threadbear", None),
            ("Extra", None),
            ("File", Some(file.clone())),
            ("Thread", Some(thread.clone())),
        ]);
        retitle(
            &bar,
            &[
                menu("Threadbear", vec![]),
                menu("File", vec![item("New Thread", "新しいスレッド")]),
                menu("Thread", vec![item("Archive", "アーカイブ")]),
            ],
        );
        assert_eq!(file.titles(), ["New Thread"]);
        assert_eq!(thread.titles(), ["Archive"]);
        assert_eq!(bar.titles(), ["Threadbear", "Extra", "File", "Thread"]);
    }
}
