// Cargo が組み立てる library のソースの写しを作る(desktop/ja/README.md)。
#[path = "../ja/mirror.rs"]
mod ja;

fn main() {
    ja::core();
}
