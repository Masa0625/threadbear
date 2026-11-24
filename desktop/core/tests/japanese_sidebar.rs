// The sidebar's rules with the screen in Japanese (desktop/ja/README.md),
// through the real functions: Japanese is on for this whole test program,
// which is why it's one of its own. The words a row draws are Japanese; a
// thread's title, a project's, a sub-agent's report and a runner's words
// stay as they are even when they read like an entry in the table; and the
// words that become a tooltip's name (the activity log's, the scripts')
// stay English, a row button's finding its Japanese by its address.
use serde_json::json;
use threadbear_core::chrono::{DateTime, FixedOffset};
use threadbear_core::inbox::{BackgroundTask, ThreadSummary};
use threadbear_core::ja;
use threadbear_core::remote::{RunnerBox, TokenNeed};
use threadbear_core::sidebar::{self as rows, RowAction, RowLook, RowMode, Subagent, TrailingKind};

fn thread(over: serde_json::Value) -> ThreadSummary {
    let mut v = json!({ "id": "t", "title": "A thread", "cwd": "/Users/alex/dev/misc", "status": "idle", "lastActivityAt": "2026-11-09T08:00:00Z", "preview": "", "lastSeq": 5, "lastSeenSeq": 5 });
    if let (Some(v), Some(over)) = (v.as_object_mut(), over.as_object()) {
        v.extend(over.clone());
    }
    serde_json::from_value(v).expect("a thread summary")
}

/// Melbourne in summer, as the sidebar's own tests have it.
fn at(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s)
        .expect("a time")
        .with_timezone(&FixedOffset::east_opt(11 * 3600).expect("an offset"))
}

fn ms(s: &str) -> i64 {
    at(s).timestamp_millis()
}

fn subagent(v: serde_json::Value) -> Subagent {
    serde_json::from_value(v).expect("a sub-agent")
}

fn tasks(list: &[(&str, &str, bool)]) -> Vec<BackgroundTask> {
    list.iter()
        .map(|(kind, description, finished)| BackgroundTask {
            kind: kind.to_string(),
            description: description.to_string(),
            finished: *finished,
        })
        .collect()
}

#[test]
fn the_words_a_row_draws_are_japanese() {
    ja::set_japanese(true);
    let now = at("2026-11-09T20:00:00+11:00");
    assert_eq!(
        rows::relative_short(ms("2026-11-09T19:59:40+11:00"), &now),
        "今"
    );
    assert_eq!(
        rows::relative_short(ms("2026-11-09T19:55:00+11:00"), &now),
        "5 分"
    );
    assert_eq!(
        rows::relative_short(ms("2026-11-09T17:00:00+11:00"), &now),
        "3 時間"
    );
    assert_eq!(
        rows::relative_short(ms("2026-11-07T20:00:00+11:00"), &now),
        "土曜"
    );
    assert_eq!(
        rows::relative_short(ms("2026-10-12T20:00:00+11:00"), &now),
        "10月12日"
    );
    assert_eq!(
        rows::relative_short(ms("2025-10-12T20:00:00+11:00"), &now),
        "2025年10月12日"
    );
    assert_eq!(
        rows::until_words(ms("2026-11-09T22:05:00+11:00"), &now),
        "午後10:05"
    );
    assert_eq!(
        rows::until_words(ms("2026-11-10T09:00:00+11:00"), &now),
        "火曜 午前9:00"
    );
    assert_eq!(rows::working_label(0, 53_000), "53 秒");
    assert_eq!(rows::working_label(0, 3_720_000), "1 時間 2 分");

    let failed = thread(json!({ "status": "error" }));
    let out = rows::trailing(
        &failed,
        &RowLook::of(&failed, Some("inbox")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(
        (out.text.as_str(), out.kind),
        ("失敗", TrailingKind::Failed)
    );
    let working = thread(json!({ "status": "running", "lastActivityAt": "2026-11-09T08:56:00Z" }));
    let out = rows::trailing(
        &working,
        &RowLook::of(&working, Some("inbox")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(out.text, "作業中 4 分");
    let room =
        thread(json!({ "needsYou": true, "waitingForRoom": { "until": "2026-11-09T18:00:00Z" } }));
    let out = rows::trailing(
        &room,
        &RowLook::of(&room, Some("inbox")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(out.text, "空きを待っています · 明日 午前5:00");
    let sleeping = thread(json!({ "snoozedUntil": "2026-11-09T22:00:00Z" }));
    let out = rows::trailing(&sleeping, &RowLook::PLAIN, RowMode::Hibernating, &now, None);
    assert_eq!(out.text, "火曜 午前9:00に戻ります");

    assert_eq!(
        rows::background_short(&tasks(&[("agent", "", false), ("agent", "", false)])),
        "サブエージェント 2 件"
    );
    assert_eq!(
        rows::background_short(&tasks(&[("agent", "", false), ("shell", "", false)])),
        "ジョブ 2 件"
    );
    assert_eq!(
        rows::background_short(&tasks(&[("agent", "", true)])),
        "仕上げ中"
    );

    let busy = thread(json!({ "status": "running" }));
    assert_eq!(
        rows::busy_prompt(&busy, true),
        (
            "アーカイブして、ターンを止めますか?".to_owned(),
            "このスレッドはまだ作業中です。今アーカイブするとターンが止まります。".to_owned()
        )
    );
    let jobs = thread(json!({ "background": [{ "kind": "shell", "description": "npm test" }] }));
    assert_eq!(
        rows::busy_prompt(&jobs, false),
        (
            "休止して、裏の作業を止めますか?".to_owned(),
            "このスレッドは、まだ裏でジョブ 1 件が動いています。今休止すると止まります。"
                .to_owned()
        )
    );

    // A Monday afternoon: the presets read in Japanese, their ids don't.
    let presets = rows::snooze_presets(&at("2026-11-09T14:00:00+11:00"));
    let titles: Vec<&str> = presets.iter().map(|p| p.title).collect();
    assert_eq!(
        titles,
        ["1 時間", "今晩 午後6時", "明日 午前9時", "次の月曜 午前9時"]
    );
    let ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
    assert_eq!(ids, ["hour", "evening", "tomorrow", "monday"]);
    assert_eq!(
        rows::snooze_date("tomorrow", &at("2026-11-09T14:00:00+11:00")),
        ms("2026-11-10T09:00:00+11:00")
    );

    let asleep = RunnerBox {
        state: "asleep".into(),
        ..Default::default()
    };
    let line = rows::box_trailing(&asleep).expect("an asleep box says so");
    assert_eq!(
        (line.text.as_str(), line.kind),
        ("眠っています", TrailingKind::Quiet)
    );

    let failed_sub =
        subagent(json!({ "id": "a", "status": "failed", "startedAt": "2026-11-09T08:58:00Z" }));
    assert_eq!(rows::subagent_detail(&failed_sub), "失敗");
    let running = subagent(
        json!({ "id": "a", "agentType": "Explore", "status": "running", "background": true, "toolUses": 4, "startedAt": "2026-11-09T08:58:00Z" }),
    );
    assert_eq!(rows::subagent_title(&running), "Explore サブエージェント");
    assert_eq!(
        rows::subagent_header_meta(Some(&running), Some("")),
        "Explore · 裏で実行中 · ツール呼び出し 4 回 · 無題 から"
    );
}

#[test]
fn a_person_s_and_a_runner_s_words_stay_as_they_are() {
    ja::set_japanese(true);
    let now = at("2026-11-09T20:00:00+11:00");
    // A sub-agent whose report, description and thread title read like
    // entries in the table.
    let done = subagent(
        json!({ "id": "a", "description": "Running", "status": "completed", "summary": "Done", "startedAt": "2026-11-09T08:58:00Z" }),
    );
    assert_eq!(rows::subagent_title(&done), "Running");
    assert_eq!(rows::subagent_detail(&done), "Done");
    assert_eq!(
        rows::subagent_header_meta(None, Some("Untitled")),
        "Untitled から"
    );
    // The thread it waits on, the one it moves to, what it wakes on.
    let waiting = thread(
        json!({ "needs": "nothing", "lastSeq": 5, "lastSeenSeq": 1, "snoozedUntil": "2026-11-10T09:00:00Z", "waitingOn": [{ "threadId": "x", "title": "Waiting for room" }] }),
    );
    let out = rows::trailing(
        &waiting,
        &RowLook::of(&waiting, Some("running")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(out.text, "\"Waiting for room\" を待っています");
    let moving = thread(json!({ "moving": { "state": "moving", "toName": "Stopped" } }));
    let out = rows::trailing(&moving, &RowLook::PLAIN, RowMode::Inbox, &now, None);
    assert_eq!(out.text, "Stopped に移しています");
    let topic = thread(
        json!({ "snoozedUntil": "9999-12-31T00:00:00.000Z", "wakeWhen": { "said": "when it's done", "topic": "Woke up" } }),
    );
    let out = rows::trailing(&topic, &RowLook::PLAIN, RowMode::Hibernating, &now, None);
    assert_eq!(out.text, "Woke up");
    // Projects named like entries keep their names, on the tile and in its letter.
    let in_projects = thread(
        json!({ "projects": [{ "id": "p1", "title": "Archive" }, { "id": "p2", "title": "Done" }] }),
    );
    assert_eq!(rows::tile_help(&in_projects), "Archive, Done");
    assert_eq!(rows::project_letter("Untitled"), "U");
    // A runner's note on a move that failed is its tooltip, as it is.
    let failed = thread(json!({ "moving": { "state": "failed", "note": "Failed" } }));
    let out = rows::trailing(&failed, &RowLook::PLAIN, RowMode::Inbox, &now, None);
    assert_eq!(out.text, "移せませんでした");
    assert_eq!(out.tip.as_deref(), Some("Failed"));
}

#[test]
fn the_words_that_name_a_tooltip_stay_english() {
    ja::set_japanese(true);
    let now = at("2026-11-09T20:00:00+11:00");
    // A row's hover buttons: the tooltip's name (and the screen reader's,
    // and the button's id) is English, and its words are found by address.
    let plain = thread(json!({}));
    let starred = thread(json!({ "starredAt": "2026-11-09T00:00:00Z" }));
    for (action, t, en, shown) in [
        (RowAction::Star, &starred, "Remove star", "スターを外す"),
        (
            RowAction::Star,
            &plain,
            "Star: keep at the top",
            "スター: 一番上に置いておく",
        ),
        (RowAction::Snooze, &plain, "Hibernate", "休止する"),
        (RowAction::Archive, &plain, "Archive", "アーカイブする"),
        (RowAction::Unsnooze, &plain, "Wake", "起こす"),
        (
            RowAction::Unarchive,
            &plain,
            "Unarchive",
            "アーカイブから戻す",
        ),
        (RowAction::Delete, &plain, "Delete chat…", "チャットを削除…"),
    ] {
        let tip = action.tip(t);
        assert_eq!(tip, en);
        assert_eq!(
            ja::entry_of(tip, &ja::TABLE).map(|e| e.1),
            Some(shown),
            "{en} finds its Japanese by its address"
        );
    }
    // Tooltips handed on as Strings can't be told by address, so they're
    // English, times and counts inside them too.
    let failed = thread(json!({ "status": "error" }));
    let out = rows::trailing(
        &failed,
        &RowLook::of(&failed, Some("inbox")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(out.tip.as_deref(), Some("The last turn failed"));
    let woke = thread(
        json!({ "woke": { "at": "2026-11-09T08:00:00Z", "since": "2026-11-07T08:00:00Z" } }),
    );
    let out = rows::trailing(
        &woke,
        &RowLook::of(&woke, Some("inbox")),
        RowMode::Inbox,
        &now,
        None,
    );
    assert_eq!(out.text, "起きました");
    assert_eq!(
        out.tip.as_deref(),
        Some("Woke up after hibernating for 2 days")
    );
    let topic = thread(
        json!({ "snoozedUntil": "2026-11-09T22:00:00Z", "wakeWhen": { "said": "next time I talk about wood", "topic": "wood" } }),
    );
    let out = rows::trailing(&topic, &RowLook::PLAIN, RowMode::Hibernating, &now, None);
    assert_eq!(out.text, "wood");
    assert_eq!(
        out.tip.as_deref(),
        Some("Back when you next talk about wood, or Tue 9:00 am at the latest. You said \"next time I talk about wood\".")
    );
    assert_eq!(
        rows::background_detail(&tasks(&[("agent", "", false), ("shell", "npm test", true)])),
        "1 sub-agent\nnpm test (finished)"
    );
    let started = thread(
        json!({ "asked": "Fix it", "startedBy": { "kind": "thread", "label": "x", "threadTitle": "Nightly" } }),
    );
    assert_eq!(rows::place_tip(&started), "You: Fix it");
    assert_eq!(
        rows::byline(&started).map(|b| b.tip).as_deref(),
        Some("Started by another thread: Nightly")
    );
    let needs = RunnerBox {
        state: "needsToken".into(),
        token: Some(TokenNeed::None),
        ..Default::default()
    };
    let line = rows::box_trailing(&needs).expect("a box waiting on a token says so");
    assert_eq!(line.text, "クラウドトークンが要ります");
    assert_eq!(
        line.tip.as_deref(),
        Some("its Claude login has no cloud token yet. Open the thread to make one; the message goes once it's set.")
    );
}
