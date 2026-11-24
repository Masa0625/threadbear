// この写しで Cargo が組み立てる library の根。中身は src/lib.rs そのままで、
// desktop/ja/core/ に表のあるファイルの文だけが日本語でも出せる。言語を選ぶ
// `threadbear_core::ja` も足す(desktop/ja/README.md)。下で取り込む写しは
// build.rs が書く。
include!(concat!(env!("OUT_DIR"), "/ja/lib.rs"));

// コンパイルされない。cargo fmt がここから src/ を整える(根が src/lib.rs
// だった時と同じに)。
#[cfg(any())]
#[path = "../src/lib.rs"]
mod formatted_by_cargo_fmt;
