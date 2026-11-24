# 画面の日本語(build の時の写し)

この写しの Mac の app は、画面の文を日本語でも出せる。元のソース(`desktop/gpui/src`・`desktop/core/src`)は 1 文字も変えない。build script が組み立ての前にソースの写しを作り、表のある文だけを「英語か日本語を選ぶ式」に差し替える。Mike の更新はそのまま入り、ぶつかるのは下の 3 か所だけ。

## 言語の決め方

値は `system`(システムに合わせる)・`en`・`ja` の 3 つ。

1. 環境変数 `THREADBEAR_LANG`
2. データのフォルダ(ランナーの home)の `desktop-language.json` の `{"language": "ja"}`
3. どちらも無ければ `system`。macOS の優先言語の一番上が `ja` で始まれば日本語。

自動操作の build(`--features automation`、specs)は、`THREADBEAR_LANG` で頼まれない限り英語。試験は app を立ち上げないので、いつも英語(`threadbear_core::ja` の既定)。決めるのは `start.rs`。

## 仕組み

- `gpui/Cargo.toml` の `[[bin]] path` と `core/Cargo.toml` の `[lib] path` が、`gpui/ja/main.rs`・`core/ja/lib.rs` を指す。どちらも写しを `include!` で取り込むだけのファイル。cargo fmt は、そこの `#[cfg(any())] mod` から元の `src/` を整える。
- `gpui/build.rs`(Windows の段と macOS の段の間に 4 行足した。Mike がよく触る `fn main` の頭を避けた)と `core/build.rs`(新しい)が `mirror.rs` を呼ぶ。写しの根は、各 `mod x;` に元のファイルの絶対パスを `#[path]` で付けた物。表のあるファイルと、子のフォルダを持つファイルだけを `$OUT_DIR/ja/` に写す。差し替えは行の中だけで、行の数は変えない(コンパイラの指す n 行目は元の n 行目)。
- 表は `ja/<crate>/<src の下のパス>.txt`。例: `ja/gpui/header.txt` は `gpui/src/header.rs` の表。
  - `"English" = "日本語"`: その literal を `threadbear_core::ja::t("English", "日本語")` に、`format!("English {x}", …)` を `ja_format!("日本語 {x}", "English {x}", …)` に差し替える。
  - `tip "English" = "日本語"`: 呼ぶ所では英語のまま。ツールチップを描く時に `ja::draw` が訳す(`{…}` を埋めた文と、行ごとの文も引ける)。ツールチップの文は活動の記録と自動操作の名前を兼ねるので、英語で渡す。
  - `replace:` と `with:`: 写しのコードの 1 か所を置き換える(描く所に `ja::draw` を差す。`ja/gpui/tips.txt`)。
- 差し替えないで英語のまま残す所(手で書かなくてよい): 試験(`#[cfg(test)]`・`#[test]`)、属性、`const`・`static`、`==` で比べる所と match の型、`tip`・`tipped!`・`aria_label`・`json!`・記録の macro などに直に渡す所。

## 表に無い文を見つける

build のたびに `$OUT_DIR/ja-report.txt` に、訳した所・英語のまま残した所・表に無い画面の文らしい literal・表だけにある文を書く。

- 表だけにある文(Mike が文を変えた・消した)と、当たらなくなった `replace:` は、毎回 `cargo:warning` に出る。
- 表に無い文も warning に出すには、`THREADBEAR_JA_REPORT=1` を付けて build する。

## 残りのファイルに広げる

1. `ja/gpui/<名前>.txt` を作る(ファイルを置くだけで写しの対象になる)。
2. `THREADBEAR_JA_REPORT=1` で check を流し、warning の「表に無い」の一覧を表に写して訳す。
3. ツールチップに流れる文(`let tip = …` から `tipped!` に渡る物、`small_button` のような helper の tip の引数)は `tip` を付ける。`ja-report.txt` の「訳した」の一覧で、記録に流れる物が無いかを見る。
