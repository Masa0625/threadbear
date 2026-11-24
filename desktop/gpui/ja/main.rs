// 取り込むファイルは `#![...]` を持てないので、src/main.rs の物をここに置く。
#![cfg_attr(
    not(feature = "automation"),
    allow(
        dead_code,
        unused_imports,
        reason = "read-outs only the scripted control uses"
    )
)]
#![cfg_attr(
    all(windows, not(debug_assertions), not(feature = "automation")),
    windows_subsystem = "windows"
)]
include!(concat!(env!("OUT_DIR"), "/ja/main.rs"));

#[path = "../../ja/language.rs"]
mod ja_language;

#[cfg(any(target_os = "macos", test))]
#[path = "../../ja/menu.rs"]
mod ja_menu;

// コンパイルされない。cargo fmt がここから src/ を整える(根が src/main.rs
// だった時と同じに)。
#[cfg(any())]
#[path = "../src/main.rs"]
mod formatted_by_cargo_fmt;
