use serde_json::json;
use threadbear_core::{ja, updates};

fn runner(v: serde_json::Value) -> updates::RunnerUpdates {
    updates::RunnerUpdates::decode(&v).expect("the runner's updates decode")
}

fn clis(reason: &str) -> Vec<updates::CliStatus> {
    updates::decode_clis(&json!([
        { "provider": "claude", "path": "/x/claude", "managedBy": "cli", "installedVersion": "2.1.311", "latestVersion": "2.1.312", "status": "behind", "policy": "auto", "inUse": [], "checkedAt": "2026-11-11T01:00:00.000Z",
          "update": { "state": "running", "targetVersion": "2.1.312" } },
        { "provider": "codex", "path": "/x/codex", "managedBy": "threadbear", "installedVersion": "0.164.0", "latestVersion": "0.165.0", "status": "held", "policy": "auto", "inUse": [], "previousVersion": "0.163.0",
          "checkedAt": "2026-11-11T01:00:00.000Z", "hold": { "version": "0.165.0", "reason": reason, "missing": [] } },
    ]))
}

fn from_mike(extra: serde_json::Value) -> updates::RunnerUpdates {
    let mut v = json!({ "state": "idle", "oldRunners": [], "upstream": {
        "state": "available", "release": { "tag": "release/2026-11-06", "changes": 12, "subjects": [] } } });
    if let (Some(v), Some(extra)) = (v.as_object_mut(), extra.as_object()) {
        v.extend(extra.clone());
    }
    runner(v)
}

#[test]
fn the_words_drawn_are_japanese() {
    ja::set_japanese(true);
    let app = updates::AppSide {
        running: Some("abc1234".into()),
        ..Default::default()
    };
    assert_eq!(updates::app_line(&app), "アプリは最新です(abc1234)。");
    let idle = runner(json!({ "state": "idle", "oldRunners": [] }));
    assert_eq!(updates::runner_lines(Some(&idle)), ["ランナーは最新です。"]);
    let mike = from_mike(json!({}));
    assert_eq!(
        updates::upstream_line(mike.upstream.as_ref()).as_deref(),
        Some("Mike の11月6日のリリースには、新しい変更 12 件があります。")
    );
    let settling = updates::AppSide {
        settling: true,
        quiet_minutes: 2,
        ..Default::default()
    };
    assert_eq!(
        updates::app_line(&settling),
        "main にアプリの新しい版があります。ビルドの前に静かな 2 分を待っています。"
    );
    let failed = runner(json!({ "state": "failed", "commit": "abc1234", "oldRunners": [] }));
    assert_eq!(
        updates::runner_lines(Some(&failed)),
        ["ランナーを main (abc1234) に更新できませんでした。"]
    );
}

#[test]
fn the_cli_lines_are_japanese_with_their_names_and_reasons_as_they_are() {
    ja::set_japanese(true);
    let clis = clis("the app-server protocol changed");
    assert_eq!(
        updates::status_line(&clis[0]),
        "Claude Code 2.1.311、2.1.312 があります"
    );
    assert_eq!(
        updates::banner_text(&clis[0]).as_deref(),
        Some("Claude Code を 2.1.312 に更新しています")
    );
    assert_eq!(
        updates::status_line(&clis[1]),
        "Codex 0.165.0 は止めてあります。理由: the app-server protocol changed"
    );
    assert_eq!(
        updates::go_back_title(&clis[1]).as_deref(),
        Some("0.163.0 に戻す")
    );
}

#[test]
fn the_counts_and_days_are_japanese() {
    ja::set_japanese(true);
    let d = updates::Digest {
        added: 5,
        changed: 23,
        fixed: 14,
        ..Default::default()
    };
    assert_eq!(updates::digest_counts(&d), "追加 5、変更 23、直し 14");
    assert_eq!(updates::release_day("release/2026-11-06", None), "11月6日");
}

#[test]
fn the_icon_tooltip_is_named_in_english_and_shown_in_japanese() {
    ja::set_japanese(true);
    let app = updates::AppSide::default();
    let mike = from_mike(json!({}));
    let clis = clis("the app-server protocol changed");
    let tip = updates::icon_tooltip(&app, Some(&mike), &clis);
    assert_eq!(
        tip,
        "Updates\n12 new changes from Mike\nUpdating Claude Code to 2.1.312\nCodex 0.165.0 is held back because the app-server protocol changed"
    );
    assert_eq!(
        ja::made_shown(&tip).as_deref(),
        Some("更新\n新しい変更 12 件(Mike から)\nClaude Code を 2.1.312 に更新しています\nCodex 0.165.0 は止めてあります。理由: the app-server protocol changed")
    );
}

#[test]
fn a_button_s_tooltip_handed_to_a_plain_helper_stays_english() {
    ja::set_japanese(true);
    let launcher = runner(
        json!({ "state": "idle", "oldRunners": [], "launcherChanged": true, "launcherStages": true }),
    );
    assert_eq!(
        updates::launcher_button(&launcher),
        Some((
            "更新する",
            "Put the new Threadbear Runner in place. Nothing stops; it starts next time your Mac restarts.",
            false
        ))
    );
}

#[test]
fn a_runner_s_words_that_read_like_the_table_stay_as_they_are() {
    ja::set_japanese(true);
    let mike = from_mike(json!({ "webRuntimeProblem": "Updates" }));
    let clis = clis("Every change");
    let tip = updates::icon_tooltip(&updates::AppSide::default(), Some(&mike), &clis);
    let shown = ja::made_shown(&tip).expect("the tooltip has its Japanese");
    assert!(shown.starts_with("更新\nUpdates\n"), "{shown}");
    assert!(shown.ends_with("理由: Every change"), "{shown}");
    assert!(updates::runner_lines(Some(&mike)).contains(&"Updates".to_owned()));
    assert_eq!(ja::made_shown("Updates"), None);
    assert_eq!(ja::made_shown("12 new changes from Mike"), None);
}
