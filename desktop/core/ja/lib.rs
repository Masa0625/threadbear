include!(concat!(env!("OUT_DIR"), "/ja/lib.rs"));

// コンパイルされない。cargo fmt がここから src/ を整える(根が src/lib.rs
// だった時と同じに)。
#[cfg(any())]
#[path = "../src/lib.rs"]
mod formatted_by_cargo_fmt;
