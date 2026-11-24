# 画面の日本語(build の時の写し)

この写しの Mac の app は、画面の文を日本語でも出せる。元のソース(`desktop/gpui/src`・`desktop/core/src`)は 1 文字も変えない。build script が組み立ての前にソースの写しを作り、表にある文だけを「英語か日本語を選ぶ式」に差し替える。Mike の更新はそのまま入り、ぶつかるのは `gpui/Cargo.toml`・`core/Cargo.toml`・`gpui/build.rs` の 3 か所だけ。

## 言語の決め方

値は `system`(システムに合わせる)・`en`・`ja` の 3 つ。

1. 環境変数 `THREADBEAR_LANG`
2. データのフォルダ(ランナーの home)の `desktop-settings.json` の `"language"`(`"appearance"` と同じファイル。app が保存しても消えない)
3. どちらも無ければ `system`。macOS の優先言語の一番上が `ja` で始まれば日本語。

自動操作の build(`--features automation`、specs)は、`THREADBEAR_LANG` で頼まれない限り英語。試験は app を立ち上げないので、いつも英語(`threadbear_core::ja` の既定)。決めるのは `start.rs`。

## 仕組み

- `gpui/Cargo.toml` の `[[bin]] path` と `core/Cargo.toml` の `[lib] path` が、`gpui/ja/main.rs`・`core/ja/lib.rs` を指す。どちらも写しを `include!` で取り込むだけのファイル。cargo fmt は、そこの `#[cfg(any())] mod` から元の `src/` を整える。
- `gpui/build.rs` と `core/build.rs` が `mirror.rs` を呼ぶ。写しの根は、各 `mod x;` に元のファイルの絶対パスを `#[path]` で付けた物。表のあるファイルと、子のフォルダを持つファイルだけを `$OUT_DIR/ja/` に写す。差し替えは行の中だけで、行の数は変えない(コンパイラの指す n 行目は元の n 行目)。
- 表は `ja/<crate>/<src の下のパス>.txt`。例: `ja/gpui/header.txt` は `gpui/src/header.rs` の表。build script は表の文を crate ごとの `TABLE`(`$OUT_DIR/ja/table.rs`・`ja_table.rs`)に書き、写しはその行を指す。
- 差し替えないで英語のまま残す所(手で書かなくてよい): 試験(`#[cfg(test)]`・`#[test]`)、自動操作だけのコード、属性、`const`・`static`、`==` で比べる所と match の型、`x["鍵"]`、`aria_label`・`id`・`icon`・`json!`・記録の macro・`split` などの文字列の照合に直に渡す所。

### 表の書き方

| 行 | 意味 |
|---|---|
| `"English" = "日本語"` | 書いた所で言語を選ぶ。literal は `t(&TABLE[i])` に、`format!("English", …)` は `ja_format!("日本語", "English", …)` になる(引数はそのまま。日本語で使わない位置の引数は `{0:.0}` で消せる)。 |
| `tip "English" = "日本語"` | 書いた所では英語のまま。id や読み上げの名前を兼ねる文に使う。描く所で `ja_tip::shown_str` が日本語にする(`replace:` で差す)。 |
| `skip "English"` | 画面に出ない文。「表に無い」の一覧から外す。理由を `#` の後に書く。 |
| `replace: <コード>` と次の `with: <コード>` | 写しのコードを 1 か所だけ置き換える。当たる数が 1 でなければ build が止まる。`replace all:` は 1 か所以上の全部。 |
| `made: <fn の名前>` | その関数の答えがツールチップになる(更新のアイコン)。英語と日本語で 1 回ずつ作る。 |
| `tip-macro: <macro> <何番目>` | その macro の何番目の引数がツールチップか(header.rs の `tipped!` は 2)。 |
| `tip-method: tip` | そのファイルの `.tip(…)` は Tip のツールチップ。二度作る形に替える。 |

## ツールチップ: 名前は英語、表示は日本語

ツールチップの文は、操作の記録(activity log)と、自動操作が部品を探す名前を兼ねる。日本語の時も名前は英語のまま、画面に描く文だけ日本語にする(`tip.rs`)。

日本語にするのは、コードが表から作った文だと、届き方で分かる物だけ。字面では見ない。だから、ランナーの文や利用者の題がたまたま表と同じ字面でも、そのまま出る。

1. **二度作る**: 写したファイルの `.tip(…)` と `tipped!` は、文を英語でもう一度作り(`ja::english`)、名前にする(`ja_tip::faces`)。変数を渡す所は、写しがその `let` を二度作る形に替える(`ja_tip::both`)。利用者の文はどちらの回も同じなので、そのまま出る。
2. **表の置き場で見分ける**: `t(&TABLE[i])` の答えは表そのものの `&'static str` なので、引数や `PushButton` を通って届いても、置き場(番地)でどの行か分かる。`push_button.txt` は、PushButton がツールチップを SharedString で持つ時に置き場を保つ(`ja_tip::keep`)。String に写されると分からなくなり、そのまま出る。表の文が、`keep` の無い helper(`impl Into<SharedString>` で受けてすぐ SharedString にする物。highlights.rs の `tipped`・`tipped_as` と、それを呼ぶ activity_panels.rs の `push_button` など)に渡ると、名前と読み上げが日本語になる。そういう文は表で `tip` にする(名前は英語、表示も英語。updates.rs の `launcher_button` のツールチップ)。helper に `keep` を差すには、受ける型に `+ AsRef<str>` が要り、それが呼ぶ側の generic な helper にも広がるので、まだしていない。
3. **`made` の答え**: 更新のアイコンのツールチップは、String と構造体を通ってレールのボタンに届く。`made:` の関数の、一番新しい答えと文全体が同じ時だけ日本語にする(`ja::made_shown`)。1 行だけや表の 1 語と同じ文は当たらない。

`.tip(…)` を二度作る形に替えるのは、表に `tip-method: tip` と書いたファイルだけ(highlights.rs の PaneRow のように、自分の `tip` を持つ型があるため)。

写しが `let` を二度作れない時(ほかにも使われる・型が書いてある・`?` や `return` で抜ける・3 つ以上の組を `match` などで作る)は、build の warning「ツールチップの注意」に出る。その時の名前は、作った時の言語になる。次の物は注意に出さない。

- 渡された値: 関数と closure の引数(`|s, tip|`)、match の腕や `if let` が取り出す値(`Some(tip) =>`)、`let tip = other;`。型が `&str` の `let` も、表の literal なら置き場で分かるので同じ。
- 3 つ以上の組を書き出した `let (a, tip, b) = (…, format!(…), …);` は、その要素だけを二度作る。
- 表の `replace:` が、その `let` の行を書き換える時。表が自分で二度作る物として扱う(settings_window.txt のアクセントの点)。

## build が止まる時と warning

- 止まる: `replace:` の当たる数が違う・`made:` の関数が無い・写しや表を書けない・`replace:` や `made:` のある表のファイルが無い・表の書き方の誤り・`src/main.rs` の `#![…]` が `gpui/ja/main.rs` と食い違う。どの表の何行目かを言う。
- warning: 表だけにある文(Mike が文を変えた・消した。英語に戻る)と、ツールチップの注意。

## 表に無い文を見つける

build のたびに `$OUT_DIR/ja-report.txt` に、訳した所・英語のまま残した所・表に無い画面の文らしい literal・表だけにある文・ツールチップの注意を書く。表に無い文には、ツールチップの文と、`added` のような小文字の 1 語も入る。画面に出ない物は表に `skip` で書いて外す。

- 表に無い文も warning に出すには、`THREADBEAR_JA_REPORT=1` を付けて build する。

## build が流れ直す時

build script は、表・写したファイル(表のあるファイルと、子のフォルダを持つファイル)・`desktop/ja` のコードが変わった時だけ流れ直す。ほかの `src/` のファイルは写しから元の場所を指すので、変えても build script は流れず、native の物も組み直さない。

穴が 1 つある: 子を持たなかったファイルに初めて `mod x;` と `x/` を足すと、build script が流れないので「file not found for module」で止まる。その時は表のファイルを 1 つ保存し直す(`touch desktop/ja/gpui/header.txt`)と直る。

## 残りのファイルに広げる

1. `ja/gpui/<名前>.txt` を作る(ファイルを置くだけで写しの対象になる)。
2. `THREADBEAR_JA_REPORT=1` で check を流し、warning の「表に無い」の一覧を表に写して訳す。画面に出ない物は `skip` にする。
3. ファイルに自分の `tipped!` があれば、`header.txt` と同じ `tip-macro:` と、macro を二度作る形にする `replace:` を足す。`.tip(…)` が Tip の物なら `tip-method: tip` を足す。id や読み上げの名前を兼ねる文と、`keep` の無い helper に渡る文は `tip` にする(描く所に `shown_str` を差せば表示は日本語)。
4. warning の「ツールチップの注意」を見て、名前が日本語になる所が無いかを確かめる。
5. ほかのファイルが、表で訳した関数の答えを英語の literal と比べていないかを探す(`relative_short` の答えを `"now"` と比べる所など)。日本語では当たらなくなるので、比べる所のファイルにも表を置き、`replace:` で比べる相手を同じ行の今の言語の字にする(`desktop/ja/core/open_anything.txt`)。

## Mac のメニューバー

メニューは英語で作って絵を付けてから、項目の題を並びの番号で日本語に書き換える(`gpui/shell.txt`)。macOS が自分の項目を足して番号がずれた所に別のコマンドの名前を付けないよう、今の題が同じ並びの英語の題と同じ項目だけを書き換え、違う項目とその子のメニューは英語のまま残す。照らし合わせは AppKit に頼らない `menu.rs` で、その試験もそこにある。
