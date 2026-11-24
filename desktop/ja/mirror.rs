// 日本語の画面のための、ソースの写し(desktop/ja/README.md)。
//
// Cargo が組み立てるのは src/ そのものではなく、build script がここで作る写し
// ($OUT_DIR/ja/)。写しの根(main.rs・lib.rs)は、各 `mod x;` に元のファイルの
// 絶対パスを `#[path]` で付けただけの物で、表(desktop/ja/<crate>/)のある
// ファイルだけを、表の英語の literal を「英語か日本語を選ぶ式」に差し替えて写す。
// 元のソースは 1 文字も変えないので、Mike の更新はそのまま入る。
//
// 差し替えは行の中だけで行い、行の数を変えない。写しのファイルの n 行目は
// 元の n 行目のままなので、コンパイラの指す行はそのまま元のファイルで引ける。
//
// build script(gpui/build.rs・core/build.rs)と、core の試験(`ja::mirror_tests`)
// から使う。std だけで書く(Cargo.lock を変えない)。
#![allow(dead_code, reason = "two build scripts each use their own half")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// gpui の build script から: src/main.rs の写しを作る。
pub fn gpui() {
    let dir = manifest_dir();
    let out = out_dir();
    let report = mirror(&Crate {
        dir: dir.clone(),
        root: "src/main.rs",
        stub: "ja/main.rs",
        tables: dir.join("../ja/gpui"),
        runtime: "threadbear_core::ja",
        format_macro: "threadbear_core::ja_format!",
        extra: vec![
            path_mod(&dir.join("../ja/start.rs"), "mod ja_start;"),
            path_mod(&out.join("ja/ja_table.rs"), "mod ja_table;"),
        ],
        main_call: Some("crate::ja_start::start(cfg!(feature = \"automation\"));".to_owned()),
        out: out.clone(),
    });
    // 描く時に引く表: ツールチップは記録と自動操作に英語で渡り、描く時だけ訳す。
    // core の文(更新のツールチップなど)も gpui が描くので、core の表も入れる。
    let mut report = report;
    let mut core = Report::default();
    load_tables(&dir.join("../ja/core"), &mut core);
    report.draw.extend(core.draw);
    report.draw.sort();
    report.draw.dedup_by(|a, b| a.0 == b.0);
    write_if_changed(&out.join("ja/ja_table.rs"), &report.draw_table());
    report.emit(&[dir.join("src"), dir.join("ja"), dir.join("../ja")]);
}

/// core の build script から: src/lib.rs の写しを作る。
pub fn core() {
    let dir = manifest_dir();
    let report = mirror(&Crate {
        dir: dir.clone(),
        root: "src/lib.rs",
        stub: "ja/lib.rs",
        tables: dir.join("../ja/core"),
        runtime: "crate::ja",
        format_macro: "crate::ja_format!",
        extra: vec![
            path_mod(&dir.join("../ja/runtime.rs"), "pub mod ja;"),
            format!(
                "#[cfg(test)] {}",
                path_mod(&dir.join("../ja/mirror.rs"), "mod ja_mirror_tests;")
            ),
        ],
        main_call: None,
        out: out_dir(),
    });
    report.emit(&[dir.join("src"), dir.join("ja"), dir.join("../ja/core"), dir.join("../ja/runtime.rs"), dir.join("../ja/mirror.rs")]);
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default())
}

fn out_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default())
}

fn path_mod(path: &Path, item: &str) -> String {
    format!("#[path = {:?}] {item}", clean(path).display().to_string())
}

/// 1 つの crate の写し方。
pub struct Crate {
    pub dir: PathBuf,
    /// 元の根(Cargo.toml が前に指していた物)。
    pub root: &'static str,
    /// Cargo.toml が今指す、写しを取り込むだけのファイル。
    pub stub: &'static str,
    pub tables: PathBuf,
    /// 差し替えた式が呼ぶ所。
    pub runtime: &'static str,
    pub format_macro: &'static str,
    /// 写しの根の終わりに足す行。
    pub extra: Vec<String>,
    /// `fn main` の始めに足す式。
    pub main_call: Option<String>,
    pub out: PathBuf,
}

/// 写した結果。表に無い文・使われなかった表の行を、取り込みの時に見せる。
#[derive(Default)]
pub struct Report {
    /// 訳した所: (ファイル, 行, 英語)。
    pub translated: Vec<(String, usize, String)>,
    /// 英語のまま残した表の文(記録と自動操作に渡る所): (ファイル, 行, 英語, 呼び先)。
    pub kept: Vec<(String, usize, String, String)>,
    /// 表に無い、画面の文らしい literal: (ファイル, 行, 英語)。
    pub missing: Vec<(String, usize, String)>,
    /// 表にあるのにソースに無い行: (表, 英語)。
    pub stale: Vec<(String, String)>,
    /// 当たらなかった置き換えの規則: (表, 探す文)。
    pub unmatched_rules: Vec<(String, String)>,
    /// 壊れていて止めるべき事。
    pub errors: Vec<String>,
    /// 描く時に引く表(ツールチップ用): (英語, 日本語)。どちらも literal の中身のまま。
    pub draw: Vec<(String, String)>,
}

impl Report {
    fn emit(&self, watch: &[PathBuf]) {
        for path in watch {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        println!("cargo:rerun-if-env-changed=THREADBEAR_JA_REPORT");
        let loud = std::env::var_os("THREADBEAR_JA_REPORT").is_some();
        for (table, en) in &self.stale {
            println!("cargo:warning=ja: {table} の \"{en}\" がソースに無い(Mike が変えたら、表を直す)");
        }
        for (table, find) in &self.unmatched_rules {
            println!("cargo:warning=ja: {table} の置き換え \"{find}\" が当たらない");
        }
        if loud {
            for (file, line, en) in &self.missing {
                println!("cargo:warning=ja: 表に無い {file}:{line} \"{en}\"");
            }
        }
        let text = self.text();
        write_if_changed(&out_dir().join("ja-report.txt"), &text);
        if !self.errors.is_empty() {
            for e in &self.errors {
                eprintln!("ja: {e}");
            }
            std::process::exit(1);
        }
    }

    /// 人が読む一覧。$OUT_DIR/ja-report.txt に書く。
    pub fn text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "訳した所 {} / 英語のまま(記録・自動操作) {} / 表に無い {} / 表だけにある {}",
            self.translated.len(),
            self.kept.len(),
            self.missing.len(),
            self.stale.len()
        );
        for (f, l, en) in &self.missing {
            let _ = writeln!(s, "表に無い\t{f}:{l}\t{en}");
        }
        for (t, en) in &self.stale {
            let _ = writeln!(s, "表だけ\t{t}\t{en}");
        }
        for (f, l, en, callee) in &self.kept {
            let _ = writeln!(s, "英語のまま\t{f}:{l}\t{callee}\t{en}");
        }
        for (f, l, en) in &self.translated {
            let _ = writeln!(s, "訳した\t{f}:{l}\t{en}");
        }
        s
    }

    fn draw_table(&self) -> String {
        let mut s = String::from(
            "// build.rs が desktop/ja/gpui/ の表から書く。描く時だけ引く(ツールチップ)。\n\
             pub(crate) static ENTRIES: &[(&str, &str)] = &[\n",
        );
        for (en, ja) in &self.draw {
            let _ = writeln!(s, "    (\"{en}\", \"{ja}\"),");
        }
        s.push_str("];\n");
        s
    }
}

/// 写しを作る。書くのは中身が変わったファイルだけ(変えなければ再 build しない)。
pub fn mirror(c: &Crate) -> Report {
    let mut report = Report::default();
    let tables = load_tables(&c.tables, &mut report);
    let mut m = Mirror {
        c,
        tables,
        report,
        written: Vec::new(),
    };
    let root = c.dir.join(c.root);
    let text = m.file(&root, true, Path::new(c.root));
    // 写しの根は include! で取り込むので、`#![...]` と `//!` を置けない。
    // それらは stub が持つ。同じかを見る(Mike が変えたら止まって知らせる)。
    let (text, inner) = blank_inner_attributes(&text);
    let stub = std::fs::read_to_string(c.dir.join(c.stub)).unwrap_or_default();
    let stub_attrs = squash(&blank_inner_attributes(&strip_comments(&stub)).1.join(""));
    for attr in &inner {
        if !stub_attrs.contains(&squash(attr)) {
            m.report.errors.push(format!(
                "{} の `{}` が {} に無い。写して揃える",
                c.root,
                attr.lines().next().unwrap_or(""),
                c.stub
            ));
        }
    }
    let mut text = text;
    text.push('\n');
    for line in &c.extra {
        text.push_str(line);
        text.push('\n');
    }
    write_if_changed(&c.out.join("ja").join(file_name(c.root)), &text);
    for (path, en) in m.unused() {
        m.report.stale.push((path, en));
    }
    m.report
}

struct Table {
    /// 表のファイルのパス(知らせる時に使う)。
    name: String,
    entries: Vec<Entry>,
    rules: Vec<(String, String, bool)>,
}

struct Entry {
    en: String,
    ja: String,
    /// 呼ぶ所では訳さず、描く時だけ訳す(ツールチップに流れる文)。
    tip: bool,
    used: bool,
}

/// desktop/ja/<crate>/<src の下のパス>.txt を、src/<パス>.rs の表として読む。
fn load_tables(dir: &Path, report: &mut Report) -> BTreeMap<PathBuf, Table> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "txt") {
                let Ok(rel) = p.strip_prefix(dir) else {
                    continue;
                };
                let src = Path::new("src").join(rel.with_extension("rs"));
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                let name = format!("desktop/ja/{}", p.strip_prefix(dir.join("..")).unwrap_or(&p).display());
                match parse_table(&name, &text) {
                    Ok(t) => {
                        for (en, ja) in t.entries.iter().map(|e| (&e.en, &e.ja)) {
                            report.draw.push((en.clone(), ja.clone()));
                        }
                        out.insert(src, t);
                    }
                    Err(e) => report.errors.push(e),
                }
            }
        }
    }
    report.draw.sort();
    report.draw.dedup_by(|a, b| a.0 == b.0);
    out
}

/// 表の書き方(1 行 1 つ):
///   "English" = "日本語"          呼ぶ所で訳す(literal の中身を、エスケープごとそのまま)
///   tip "English {}" = "{} の…"   呼ぶ所では訳さず、ツールチップを描く時だけ訳す
///   replace: <元のコード>         次の行の with: に置き換える(描く所に引く式を差す)
///   with: <新しいコード>
///   # から行の終わりは注釈
fn parse_table(name: &str, text: &str) -> Result<Table, String> {
    let mut t = Table {
        name: name.to_owned(),
        entries: Vec::new(),
        rules: Vec::new(),
    };
    let mut pending: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let at = || format!("{name}:{}", i + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(code) = line.strip_prefix("replace:") {
            pending = Some(code.trim().to_owned());
            continue;
        }
        if let Some(code) = line.strip_prefix("with:") {
            let find = pending.take().ok_or_else(|| format!("{}: with: の前に replace: が無い", at()))?;
            t.rules.push((find, code.trim().to_owned(), false));
            continue;
        }
        let (tip, rest) = match line.strip_prefix("tip ") {
            Some(r) => (true, r.trim_start()),
            None => (false, line),
        };
        let (en, rest) = literal(rest).ok_or_else(|| format!("{}: 英語の \"…\" が読めない", at()))?;
        let rest = rest.trim_start().strip_prefix('=').ok_or_else(|| format!("{}: = が無い", at()))?;
        let (ja, rest) = literal(rest.trim_start()).ok_or_else(|| format!("{}: 日本語の \"…\" が読めない", at()))?;
        let rest = rest.trim();
        if !(rest.is_empty() || rest.starts_with('#')) {
            return Err(format!("{}: 行の終わりに余分な物", at()));
        }
        if ja.contains('—') {
            return Err(format!("{}: 日本語の UI の文に em dash を使わない", at()));
        }
        t.entries.push(Entry {
            en: en.to_owned(),
            ja: ja.to_owned(),
            tip,
            used: false,
        });
    }
    if pending.is_some() {
        return Err(format!("{name}: replace: の後に with: が無い"));
    }
    Ok(t)
}

/// `"…"` の中身(エスケープはそのまま)と残り。
fn literal(s: &str) -> Option<(&str, &str)> {
    let body = s.strip_prefix('"')?;
    let mut escaped = false;
    for (i, ch) in body.char_indices() {
        match ch {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return Some((&body[..i], &body[i + 1..])),
            _ => escaped = false,
        }
    }
    None
}

struct Mirror<'a> {
    c: &'a Crate,
    tables: BTreeMap<PathBuf, Table>,
    report: Report,
    written: Vec<PathBuf>,
}

impl Mirror<'_> {
    /// `path`(元のファイル)を写した中身。`rel` は crate の中のパス。
    fn file(&mut self, path: &Path, is_root: bool, rel: &Path) -> String {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let toks = lex(&text);
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        // mod-rs(根と mod.rs)の子は同じフォルダ、ほかは名前のフォルダ。
        let mod_rs = is_root || path.file_name().is_some_and(|n| n == "mod.rs");
        let own_dir = match (mod_rs, path.parent(), path.file_stem()) {
            (true, Some(p), _) => p.to_path_buf(),
            (false, Some(p), Some(stem)) => p.join(stem),
            _ => PathBuf::new(),
        };
        self.child_mods(&text, &toks, path, &own_dir, &mut edits);
        self.include_paths(&text, &toks, path, &mut edits);
        if let Some(table) = self.tables.remove(rel) {
            let mut table = table;
            self.translate(&text, &toks, rel, &mut table, &mut edits);
            self.tables.insert(rel.to_path_buf(), table);
        }
        if is_root {
            if let Some(call) = &self.c.main_call {
                if !main_call(&text, &toks, call, &mut edits) {
                    self.report.errors.push(format!("{} に `fn main() {{` が無い", self.c.root));
                }
            }
        }
        let out = apply(&text, edits);
        if let Some(table) = self.tables.get_mut(rel) {
            let mut out = out;
            for rule in &mut table.rules {
                if let Some(at) = out.find(&rule.0) {
                    out.replace_range(at..at + rule.0.len(), &rule.1);
                    rule.2 = true;
                }
            }
            return out;
        }
        out
    }

    /// 各 `mod x;` に、元のファイルの絶対パス(か写しのパス)を付ける。
    fn child_mods(&mut self, text: &str, toks: &[Tok], path: &Path, own_dir: &Path, edits: &mut Vec<(usize, usize, String)>) {
        // 中に書いた `mod a { mod b; }` の b は own_dir/a/b.rs。
        let mut inline: Vec<Option<String>> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            match t.kind {
                Kind::Open('{') => {
                    let name = (i >= 2 && toks[i - 2].is_ident(text, "mod")).then(|| toks[i - 1].text(text).to_owned());
                    inline.push(name);
                }
                Kind::Close('}') => {
                    inline.pop();
                }
                Kind::Ident if t.text(text) == "mod" && i + 2 < toks.len() && toks[i + 1].kind == Kind::Ident && toks[i + 2].is_punct(text, ";") => {
                    let name = toks[i + 1].text(text);
                    let mut dir = own_dir.to_path_buf();
                    for n in inline.iter().flatten() {
                        dir.push(n);
                    }
                    let start = item_start(text, toks, i);
                    let given = path_attribute(text, toks, start);
                    let file = match &given {
                        // `#[path = "…"]` は、中に書いた mod の中なら dir から、
                        // 外ならファイルのあるフォルダから(Rust の決まり)。
                        Some((_, p)) if inline.iter().flatten().next().is_some() => dir.join(p),
                        Some((_, p)) => path.parent().unwrap_or(Path::new("")).join(p),
                        None => {
                            let a = dir.join(format!("{name}.rs"));
                            if a.exists() {
                                a
                            } else {
                                dir.join(name).join("mod.rs")
                            }
                        }
                    };
                    let child_rel = rel_in_crate(&self.c.dir, &file);
                    // 子を持つ x.rs(x/ がある)と表のあるファイルは写す。ほかは元を指す。
                    let has_children = file.file_name().is_some_and(|n| n != "mod.rs") && file.with_extension("").is_dir();
                    let target = if has_children || self.tables.contains_key(&child_rel) {
                        let body = self.file(&file, false, &child_rel);
                        let dest = self.c.out.join("ja").join(&child_rel);
                        write_if_changed(&dest, &body);
                        self.written.push(dest.clone());
                        dest
                    } else {
                        file
                    };
                    let attr = format!("#[path = {:?}] ", clean(&target).display().to_string());
                    match given {
                        Some(((s, e), _)) => edits.push((s, e, attr.trim_end().to_owned())),
                        None => edits.push((toks[start].start, toks[start].start, attr)),
                    }
                    i += 3;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// 写しは別の所に置くので、`include_str!("相対")` などを絶対パスにする。
    fn include_paths(&self, text: &str, toks: &[Tok], path: &Path, edits: &mut Vec<(usize, usize, String)>) {
        let dir = path.parent().unwrap_or(Path::new(""));
        for i in 0..toks.len().saturating_sub(3) {
            let name = toks[i].text(text);
            if toks[i].kind == Kind::Ident
                && matches!(name, "include_str" | "include_bytes" | "include")
                && toks[i + 1].is_punct(text, "!")
                && toks[i + 2].kind == Kind::Open('(')
                && toks[i + 3].kind == Kind::Str
            {
                let Some(inner) = quoted(toks[i + 3].text(text)) else {
                    continue;
                };
                if !Path::new(inner).is_absolute() {
                    let abs = clean(&dir.join(inner)).display().to_string();
                    edits.push((toks[i + 3].start, toks[i + 3].end, format!("{abs:?}")));
                }
            }
        }
    }

    fn translate(&mut self, text: &str, toks: &[Tok], rel: &Path, table: &mut Table, edits: &mut Vec<(usize, usize, String)>) {
        let file = rel.display().to_string();
        let index: BTreeMap<String, usize> = table.entries.iter().enumerate().map(|(i, e)| (e.en.clone(), i)).collect();
        let mut walk = Walk::default();
        for (i, t) in toks.iter().enumerate() {
            walk.step(text, toks, i);
            if t.kind != Kind::Str {
                continue;
            }
            let lit = t.text(text);
            // r"" b"" と閉じていない物は扱わない。
            let Some(content) = quoted(lit) else {
                continue;
            };
            let line = text[..t.start].matches('\n').count() + 1;
            let ctx = walk.context(text, toks, i);
            let Some(&ei) = index.get(content) else {
                if ctx.ui && ctx.keep.is_none() && looks_like_ui(content) {
                    self.report.missing.push((file.clone(), line, content.to_owned()));
                }
                continue;
            };
            let entry = &mut table.entries[ei];
            entry.used = true;
            if !ctx.ui {
                continue;
            }
            if entry.tip || ctx.keep.is_some() {
                let why = ctx.keep.clone().unwrap_or_else(|| "tip".to_owned());
                self.report.kept.push((file.clone(), line, content.to_owned(), why));
                continue;
            }
            if let Some((mac_start, open_end)) = ctx.format_head {
                // format!("en", …) → ja_format!("ja", "en", …)。引数はそのまま。
                edits.push((mac_start, open_end, format!("{}(\"{}\", ", self.c.format_macro, entry.ja)));
            } else {
                edits.push((t.start, t.end, format!("{}::t({lit}, \"{}\")", self.c.runtime, entry.ja)));
            }
            self.report.translated.push((file.clone(), line, content.to_owned()));
        }
    }

    fn unused(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for t in self.tables.values() {
            for e in &t.entries {
                if !e.used {
                    out.push((t.name.clone(), e.en.clone()));
                }
            }
            for r in &t.rules {
                if !r.2 {
                    out.push((t.name.clone(), format!("replace: {}", r.0)));
                }
            }
        }
        out
    }
}

fn rel_in_crate(dir: &Path, file: &Path) -> PathBuf {
    let f = clean(file);
    f.strip_prefix(clean(dir)).map(Path::to_path_buf).unwrap_or(f)
}

/// `..` と `.` を畳んだパス(ファイルが無くても使える)。
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// `"…"` の literal の中身。r"" b"" と閉じていない物は None。
fn quoted(lit: &str) -> Option<&str> {
    lit.strip_prefix('"')?.strip_suffix('"')
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn write_if_changed(path: &Path, text: &str) {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(path, text) {
        println!("cargo:warning=ja: {} を書けない: {e}", path.display());
    }
}

/// item の始め(属性と pub(…) の前ではなく、pub の所)。`mod` の位置から戻る。
fn item_start(text: &str, toks: &[Tok], mod_at: usize) -> usize {
    let mut i = mod_at;
    if i > 0 && toks[i - 1].kind == Kind::Close(')') {
        let mut depth = 0;
        let mut j = i - 1;
        loop {
            match toks[j].kind {
                Kind::Close(')') => depth += 1,
                Kind::Open('(') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            if j == 0 {
                break;
            }
            j -= 1;
        }
        i = j;
    }
    if i > 0 && toks[i - 1].is_ident(text, "pub") {
        i -= 1;
    }
    i
}

/// item の前の `#[path = "…"]`: ((literal の始め, 終わり), 中身)。
fn path_attribute(text: &str, toks: &[Tok], start: usize) -> Option<((usize, usize), String)> {
    // 戻りながら、属性 `# [ … ]` を 1 つずつ見る。
    let mut end = start;
    while end >= 1 && toks[end - 1].kind == Kind::Close(']') {
        let mut j = end - 1;
        let mut depth = 0;
        loop {
            match toks[j].kind {
                Kind::Close(']') => depth += 1,
                Kind::Open('[') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            if j == 0 {
                return None;
            }
            j -= 1;
        }
        if j == 0 || !toks[j - 1].is_punct(text, "#") {
            return None;
        }
        if toks.get(j + 1).is_some_and(|t| t.is_ident(text, "path")) && toks.get(j + 3).is_some_and(|t| t.kind == Kind::Str) {
            let inner = quoted(toks[j + 3].text(text))?;
            return Some(((toks[j - 1].start, toks[end - 1].end), inner.to_owned()));
        }
        end = j - 1;
    }
    None
}

/// 根の一番外の `fn main() {` の `{` の後ろに、式を同じ行に足す。
fn main_call(text: &str, toks: &[Tok], call: &str, edits: &mut Vec<(usize, usize, String)>) -> bool {
    let mut depth = 0i32;
    for i in 0..toks.len() {
        if depth == 0
            && i + 4 < toks.len()
            && toks[i].is_ident(text, "fn")
            && toks[i + 1].is_ident(text, "main")
            && toks[i + 2].kind == Kind::Open('(')
            && toks[i + 3].kind == Kind::Close(')')
            && toks[i + 4].kind == Kind::Open('{')
        {
            let at = toks[i + 4].end;
            edits.push((at, at, format!(" {call}")));
            return true;
        }
        match toks[i].kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth -= 1,
            _ => {}
        }
    }
    false
}

fn apply(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|e| (e.0, e.1));
    let mut out = String::with_capacity(text.len() + edits.len() * 40);
    let mut at = 0;
    for (s, e, with) in edits {
        if s < at {
            continue; // 重なった差し替えは前の物を残す
        }
        out.push_str(&text[at..s]);
        out.push_str(&with);
        at = e;
    }
    out.push_str(&text[at..]);
    out
}

/// 根の `#![…]` と `//!` を空白にした中身(行と桁は保つ)と、外した属性。
fn blank_inner_attributes(text: &str) -> (String, Vec<String>) {
    let toks = lex(text);
    let mut out: Vec<u8> = text.as_bytes().to_vec();
    let mut attrs = Vec::new();
    let mut i = 0;
    // 一番外の物だけ(中の `mod x { #![…] }` は残す)。
    let mut outer = 0i32;
    while i + 2 < toks.len() {
        match toks[i].kind {
            Kind::Open(_) => outer += 1,
            Kind::Close(_) => outer -= 1,
            _ => {}
        }
        if outer == 0 && toks[i].is_punct(text, "#") && toks[i + 1].is_punct(text, "!") && toks[i + 2].kind == Kind::Open('[') {
            let mut depth = 0;
            let mut j = i + 2;
            while j < toks.len() {
                match toks[j].kind {
                    Kind::Open(_) => depth += 1,
                    Kind::Close(_) => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let (s, e) = (toks[i].start, toks.get(j).map_or(text.len(), |t| t.end));
            attrs.push(text[s..e].to_owned());
            for b in &mut out[s..e] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    let mut s = String::from_utf8(out).unwrap_or_default();
    // `//!` は行ごと注釈にする。
    s = s
        .lines()
        .map(|l| if l.trim_start().starts_with("//!") { l.replacen("//!", "// ", 1) } else { l.to_owned() })
        .collect::<Vec<_>>()
        .join("\n");
    (s, attrs)
}

fn strip_comments(text: &str) -> String {
    text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<String>()
}

/// 表に無い時に「画面の文らしい」と知らせる物: 大文字で始まり、小文字を含む。
fn looks_like_ui(content: &str) -> bool {
    let starts_upper = content.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    let words = content.contains(' ') && content.chars().any(|c| c.is_ascii_lowercase());
    let one_word = content.chars().count() >= 2 && content.chars().skip(1).all(|c| c.is_ascii_lowercase());
    starts_upper && (words || one_word) && !content.contains("::")
}

// ── 字句 ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Ident,
    Str,
    Char,
    Lifetime,
    Num,
    Punct,
    Open(char),
    Close(char),
}

#[derive(Clone, Copy, Debug)]
pub struct Tok {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
}

impl Tok {
    fn text<'t>(&self, text: &'t str) -> &'t str {
        &text[self.start..self.end]
    }
    fn is_ident(&self, text: &str, w: &str) -> bool {
        self.kind == Kind::Ident && self.text(text) == w
    }
    fn is_punct(&self, text: &str, p: &str) -> bool {
        self.kind == Kind::Punct && self.text(text) == p
    }
}

/// Rust の字句を、注釈を飛ばして並べる(literal の位置が要るだけの、軽い物)。
pub fn lex(text: &str) -> Vec<Tok> {
    let b = text.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    let push = |toks: &mut Vec<Tok>, kind, start, end| toks.push(Tok { kind, start, end });
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if let Some(end) = raw_string(b, i) {
            push(&mut toks, Kind::Str, i, end);
            i = end;
        } else if c == b'"' || ((c == b'b' || c == b'c') && b.get(i + 1) == Some(&b'"')) {
            let start = i;
            i += if c == b'"' { 1 } else { 2 };
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            push(&mut toks, Kind::Str, start, i.min(b.len()));
        } else if c == b'\'' || (c == b'b' && b.get(i + 1) == Some(&b'\'')) {
            let start = i;
            let q = if c == b'b' { i + 1 } else { i };
            // 'a' か '\n' なら文字、'a(閉じない)なら lifetime。
            let after = q + 1;
            if b.get(after) == Some(&b'\\') {
                let mut j = after + 2;
                while j < b.len() && b[j] != b'\'' {
                    j += 1;
                }
                push(&mut toks, Kind::Char, start, j + 1);
                i = j + 1;
            } else {
                let ch_len = text[after..].chars().next().map_or(1, char::len_utf8);
                if b.get(after + ch_len) == Some(&b'\'') {
                    push(&mut toks, Kind::Char, start, after + ch_len + 1);
                    i = after + ch_len + 1;
                } else {
                    let mut j = after;
                    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                        j += 1;
                    }
                    push(&mut toks, Kind::Lifetime, start, j);
                    i = j;
                }
            }
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let start = i;
            if c == b'r' && b.get(i + 1) == Some(&b'#') {
                i += 2;
            }
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 0x80) {
                i += 1;
            }
            push(&mut toks, Kind::Ident, start, i);
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || (b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))) {
                i += 1;
            }
            push(&mut toks, Kind::Num, start, i);
        } else if matches!(c, b'(' | b'[' | b'{') {
            push(&mut toks, Kind::Open(c as char), i, i + 1);
            i += 1;
        } else if matches!(c, b')' | b']' | b'}') {
            push(&mut toks, Kind::Close(c as char), i, i + 1);
            i += 1;
        } else {
            let two = text.get(i..i + 2).unwrap_or("");
            let len = if matches!(two, "=>" | "==" | "!=" | "::" | "->" | "&&" | "||" | "<=" | ">=") { 2 } else { 1 };
            let len = if !text.is_char_boundary(i + len) { 1 } else { len };
            push(&mut toks, Kind::Punct, i, i + len);
            i += len;
        }
    }
    toks
}

/// r"…" r#"…"# br"…" の終わり。
fn raw_string(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    if b.get(j) == Some(&b'b') || b.get(j) == Some(&b'c') {
        j += 1;
    }
    if b.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while b.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if b.get(j) != Some(&b'"') {
        return None;
    }
    j += 1;
    loop {
        if j >= b.len() {
            return Some(b.len());
        }
        if b[j] == b'"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&b'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
}

// ── literal の置かれた所 ───────────────────────────────────────────────

/// literal の周り: 訳してよいか、英語のまま渡す所か、format! の書式か。
pub struct Context {
    /// 画面の文が来うる所(試験・属性・const・比べる所・型の外)。
    pub ui: bool,
    /// 英語のまま渡す呼び先(ツールチップ・読み上げの名前・記録・JSON)。
    pub keep: Option<String>,
    /// format!(…) の最初の引数なら、(`format` の始め, `(` の終わり)。
    pub format_head: Option<(usize, usize)>,
}

#[derive(Default)]
struct Walk {
    frames: Vec<Frame>,
    /// `#[cfg(test)]` か `#[test]` の後で、次の `{` を飛ばす。
    test_next: bool,
    /// const・static の item の中(`;` まで)。深さを持つ。
    const_depth: Option<usize>,
}

struct Frame {
    open: char,
    callee: Option<String>,
    is_macro: bool,
    macro_start: usize,
    open_end: usize,
    arg: usize,
    skip: bool,
    attr: bool,
}

/// ここに直に渡る文は、英語のまま(画面に描く時だけ訳す・または訳さない)。
const KEEP: &[&str] = &[
    // ツールチップ: 記録と自動操作の名前を兼ねる(tips.rs)。描く時に訳す。
    "tip", "tipped", "tooltip", "aria_label",
    // 画面の外: 記録・JSON・id・試験・止まる時の文。
    "json", "id", "key_context", "debug", "info", "warn", "error", "trace", "log", "panic", "assert", "assert_eq", "assert_ne", "debug_assert", "expect", "println", "eprintln", "unreachable", "matches", "include_str", "env", "concat", "starts_with", "ends_with", "contains", "strip_prefix", "eq",
];

impl Walk {
    fn step(&mut self, text: &str, toks: &[Tok], i: usize) {
        let t = &toks[i];
        match t.kind {
            Kind::Open(open) => {
                let prev = i.checked_sub(1).map(|p| &toks[p]);
                let (callee, is_macro, macro_start) = match prev {
                    Some(p) if p.is_punct(text, "!") && i >= 2 && toks[i - 2].kind == Kind::Ident => (Some(toks[i - 2].text(text).to_owned()), true, toks[i - 2].start),
                    Some(p) if p.kind == Kind::Ident && open == '(' => (Some(p.text(text).to_owned()), false, p.start),
                    _ => (None, false, t.start),
                };
                let attr = open == '[' && prev.is_some_and(|p| p.is_punct(text, "#") || p.is_punct(text, "!") && i >= 2 && toks[i - 2].is_punct(text, "#"));
                let parent_skip = self.frames.last().is_some_and(|f| f.skip);
                let skip = parent_skip || (open == '{' && self.test_next);
                if open == '{' {
                    self.test_next = false;
                }
                if attr {
                    // #[cfg(test)]・#[test]・#[cfg(feature = "automation")](自動操作の
                    // build だけの物。そこではいつも英語)。
                    let body: String = toks[i + 1..].iter().take(6).map(|t| t.text(text)).collect();
                    if body.starts_with("cfg(test)") || body.starts_with("test]") || body.starts_with("cfg(feature=\"automation\")") {
                        self.test_next = true;
                    }
                }
                self.frames.push(Frame {
                    open,
                    callee,
                    is_macro,
                    macro_start,
                    open_end: t.end,
                    arg: 0,
                    skip,
                    attr,
                });
            }
            Kind::Close(_) => {
                self.frames.pop();
                if self.const_depth.is_some_and(|d| d > self.frames.len()) {
                    self.const_depth = None;
                }
            }
            Kind::Punct if t.text(text) == "," => {
                if let Some(f) = self.frames.last_mut() {
                    f.arg += 1;
                }
            }
            Kind::Punct if t.text(text) == ";" => {
                if self.const_depth == Some(self.frames.len()) {
                    self.const_depth = None;
                }
                if self.frames.last().is_none_or(|f| f.open == '{') {
                    self.test_next = false;
                }
            }
            Kind::Ident if matches!(t.text(text), "const" | "static") => {
                let next_fn = toks.get(i + 1).is_some_and(|n| n.is_ident(text, "fn") || n.is_ident(text, "unsafe"));
                let in_braces = self.frames.last().is_none_or(|f| f.open == '{');
                if !next_fn && in_braces && self.const_depth.is_none() {
                    self.const_depth = Some(self.frames.len());
                }
            }
            _ => {}
        }
    }

    fn context(&self, text: &str, toks: &[Tok], i: usize) -> Context {
        let prev = i.checked_sub(1).map(|p| toks[p].text(text)).unwrap_or("");
        let next = toks.get(i + 1).map_or("", |t| t.text(text));
        let in_skip = self.frames.iter().any(|f| f.skip || f.attr);
        let compared = matches!(prev, "==" | "!=") || matches!(next, "==" | "!=" | "=>" | "|");
        let ui = !in_skip && self.const_depth.is_none() && !compared;
        // 一番近い呼び出し(`{` と `[` は通り抜ける)。
        let call = self.frames.iter().rev().find(|f| f.open == '(' || f.is_macro);
        let keep = call.and_then(|f| f.callee.clone()).filter(|c| KEEP.contains(&c.as_str()));
        let format_head = self.frames.last().and_then(|f| {
            (f.is_macro && f.callee.as_deref() == Some("format") && f.arg == 0).then_some((f.macro_start, f.open_end))
        });
        Context { ui, keep, format_head }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(src: &str, table: &str) -> (String, Report) {
        let dir = std::env::temp_dir().join(format!("ja-mirror-{}-{}", std::process::id(), src.len() + table.len()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("ja")).unwrap();
        std::fs::create_dir_all(dir.join("tables")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub mod a;\n").unwrap();
        std::fs::write(dir.join("src/a.rs"), src).unwrap();
        std::fs::write(dir.join("ja/lib.rs"), "").unwrap();
        std::fs::write(dir.join("tables/a.txt"), table).unwrap();
        let c = Crate {
            dir: dir.clone(),
            root: "src/lib.rs",
            stub: "ja/lib.rs",
            tables: dir.join("tables"),
            runtime: "crate::ja",
            format_macro: "crate::ja_format!",
            extra: vec![],
            main_call: None,
            out: dir.join("out"),
        };
        let report = mirror(&c);
        let out = std::fs::read_to_string(dir.join("out/ja/src/a.rs")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        (out, report)
    }

    #[test]
    fn a_literal_on_screen_picks_its_language() {
        let (out, r) = rewrite("fn f() -> &'static str {\n    \"Dismiss\"\n}\n", "\"Dismiss\" = \"閉じる\"\n");
        assert_eq!(out, "fn f() -> &'static str {\n    crate::ja::t(\"Dismiss\", \"閉じる\")\n}\n");
        assert_eq!(r.translated.len(), 1);
    }

    #[test]
    fn a_format_keeps_its_arguments_and_its_lines() {
        let src = "fn f(t: &str) -> String {\n    format!(\n        \"Open {t}\",\n    )\n}\n";
        let (out, _) = rewrite(src, "\"Open {t}\" = \"{t} を開く\"\n");
        assert_eq!(out.lines().count(), src.lines().count());
        assert!(out.contains("crate::ja_format!(\"{t} を開く\", \n        \"Open {t}\","), "{out}");
    }

    #[test]
    fn tooltips_logs_tests_and_comparisons_stay_english() {
        let src = "fn f(e: E) -> E {\n    e.tip(\"Copy\").child(\"Copy\")\n}\nfn g(x: &str) -> bool { x == \"Copy\" }\nconst C: &str = \"Copy\";\n#[cfg(test)]\nmod tests {\n    fn h() { \"Copy\"; }\n}\n";
        let (out, r) = rewrite(src, "\"Copy\" = \"コピー\"\n");
        assert!(out.contains("e.tip(\"Copy\").child(crate::ja::t(\"Copy\", \"コピー\"))"), "{out}");
        assert!(out.contains("x == \"Copy\""));
        assert!(out.contains("const C: &str = \"Copy\";"));
        assert!(out.contains("fn h() { \"Copy\"; }"));
        assert_eq!(r.translated.len(), 1);
        assert_eq!(r.kept.len(), 1);
    }

    #[test]
    fn a_tip_entry_is_left_for_the_tooltip_to_draw() {
        let (out, r) = rewrite("fn f(p: &str) -> String { format!(\"In {}\", p) }\n", "tip \"In {}\" = \"{} の中\"\n");
        assert!(out.contains("format!(\"In {}\", p)"));
        assert_eq!(r.draw, vec![("In {}".to_owned(), "{} の中".to_owned())]);
    }

    #[test]
    fn new_words_and_gone_words_are_reported() {
        let (_, r) = rewrite("fn f(e: E) -> E { e.child(\"Brand new words\") }\n", "\"Not there any more\" = \"もう無い\"\n");
        assert_eq!(r.missing.len(), 1);
        assert_eq!(r.missing[0].2, "Brand new words");
        assert_eq!(r.stale.len(), 1);
    }

    #[test]
    fn a_table_rejects_an_em_dash() {
        assert!(parse_table("t", "\"A\" = \"あ—い\"\n").is_err());
    }
}
