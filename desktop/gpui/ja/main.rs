// この写しで Cargo が組み立てる app の根。中身は src/main.rs そのままで、
// desktop/ja/gpui/ に表のあるファイルの文だけが日本語でも出せる
// (desktop/ja/README.md)。下で取り込む写しは build.rs が書く。
//
// 取り込むファイルは `#![...]` を持てないので、src/main.rs の物をここに置く。
// 食い違えば build が止まって知らせる。
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

// コンパイルされない。cargo fmt がここから src/ を整える(根が src/main.rs
// だった時と同じに)。
#[cfg(any())]
#[path = "../src/main.rs"]
mod formatted_by_cargo_fmt;
