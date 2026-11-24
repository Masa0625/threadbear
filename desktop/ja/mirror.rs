#![allow(
    dead_code,
    reason = "each build script uses its own half, and the tests the rest"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub fn gpui() {
    let dir = manifest_dir();
    let out = out_dir();
    let mut report = mirror(&Crate {
        dir: dir.clone(),
        source_root: "src/main.rs",
        cargo_root: "ja/main.rs",
        tables: dir.join("../ja/gpui"),
        side: Side::Gpui,
        first_line_prefix: String::new(),
        extra: vec![
            path_mod(&dir.join("../ja/start.rs"), "mod ja_start;"),
            path_mod(&dir.join("../ja/tip.rs"), "mod ja_tip;"),
            path_mod(&out.join("ja/ja_table.rs"), "mod ja_table;"),
        ],
        main_call: Some("crate::ja_start::start(cfg!(feature = \"automation\"));".to_owned()),
        out,
    });
    for own in [
        "ja/main.rs",
        "../ja/mirror.rs",
        "../ja/start.rs",
        "../ja/tip.rs",
    ] {
        report.watch.push(dir.join(own));
    }
    report.emit();
}

pub fn core() {
    let dir = manifest_dir();
    let mut report = mirror(&Crate {
        dir: dir.clone(),
        source_root: "src/lib.rs",
        cargo_root: "ja/lib.rs",
        tables: dir.join("../ja/core"),
        side: Side::Core,
        // The copy's root is inside include!, where `crate::ja_format!` can't
        // be named (a rustc rule), so `#[macro_use]` lets it go by its name.
        first_line_prefix: format!(
            "#[macro_use] {} ",
            path_mod(&dir.join("../ja/runtime.rs"), "pub mod ja;")
        ),
        extra: vec![format!(
            "#[cfg(test)] {}",
            path_mod(&dir.join("../ja/mirror.rs"), "mod ja_mirror_tests;")
        )],
        main_call: None,
        out: out_dir(),
    });
    for own in ["ja/lib.rs", "../ja/runtime.rs", "../ja/mirror.rs"] {
        report.watch.push(dir.join(own));
    }
    report.emit();
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Core,
    Gpui,
}

impl Side {
    fn runtime(self) -> &'static str {
        match self {
            Side::Core => "crate::ja",
            Side::Gpui => "threadbear_core::ja",
        }
    }

    fn table(self) -> &'static str {
        match self {
            Side::Core => "crate::ja::TABLE",
            Side::Gpui => "crate::ja_table::TABLE",
        }
    }

    fn format_macro(self) -> &'static str {
        match self {
            Side::Core => "ja_format!",
            Side::Gpui => "threadbear_core::ja_format!",
        }
    }

    fn table_file(self) -> (&'static str, &'static str) {
        match self {
            Side::Core => ("table.rs", "pub static"),
            Side::Gpui => ("ja_table.rs", "pub(crate) static"),
        }
    }
}

pub struct Crate {
    pub dir: PathBuf,
    pub source_root: &'static str,
    pub cargo_root: &'static str,
    pub tables: PathBuf,
    pub side: Side,
    pub first_line_prefix: String,
    pub extra: Vec<String>,
    pub main_call: Option<String>,
    pub out: PathBuf,
}

#[derive(Default)]
pub struct Report {
    pub translated: Vec<Spot>,
    pub kept: Vec<Kept>,
    pub missing: Vec<Spot>,
    pub stale: Vec<TableOnly>,
    pub notes: Vec<TipNote>,
    pub errors: Vec<String>,
    pub watch: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct Spot {
    pub file: String,
    pub line: usize,
    pub en: String,
}

#[derive(Debug)]
pub struct Kept {
    pub file: String,
    pub line: usize,
    pub en: String,
    pub why: String,
}

#[derive(Debug)]
pub struct TableOnly {
    pub table: String,
    pub entry: String,
}

#[derive(Debug)]
pub struct TipNote {
    pub file: String,
    pub line: usize,
    pub why: String,
}

impl Report {
    fn emit(&self) {
        for path in &self.watch {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        println!("cargo:rerun-if-env-changed=THREADBEAR_JA_REPORT");
        for TableOnly { table, entry } in &self.stale {
            println!(
                "cargo:warning=ja: {table} の \"{entry}\" がソースに無い(Mike が変えたら、表を直す)"
            );
        }
        for TipNote { file, line, why } in &self.notes {
            println!("cargo:warning=ja: {file}:{line} {why}");
        }
        if std::env::var_os("THREADBEAR_JA_REPORT").is_some() {
            for Spot { file, line, en } in &self.missing {
                println!("cargo:warning=ja: 表に無い {file}:{line} \"{en}\"");
            }
        }
        let mut errors = self.errors.clone();
        if let Err(e) = write_if_changed(&out_dir().join("ja-report.txt"), &self.text()) {
            errors.push(e);
        }
        if !errors.is_empty() {
            for e in &errors {
                eprintln!("ja: {e}");
            }
            std::process::exit(1);
        }
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "訳した所 {} / 英語のまま(記録・自動操作) {} / 表に無い {} / 表だけにある {} / ツールチップの注意 {}",
            self.translated.len(),
            self.kept.len(),
            self.missing.len(),
            self.stale.len(),
            self.notes.len()
        );
        for Spot { file, line, en } in &self.missing {
            let _ = writeln!(s, "表に無い\t{file}:{line}\t{en}");
        }
        for TableOnly { table, entry } in &self.stale {
            let _ = writeln!(s, "表だけ\t{table}\t{entry}");
        }
        for TipNote { file, line, why } in &self.notes {
            let _ = writeln!(s, "注意\t{file}:{line}\t{why}");
        }
        for Kept {
            file,
            line,
            en,
            why,
        } in &self.kept
        {
            let _ = writeln!(s, "英語のまま\t{file}:{line}\t{why}\t{en}");
        }
        for Spot { file, line, en } in &self.translated {
            let _ = writeln!(s, "訳した\t{file}:{line}\t{en}");
        }
        s
    }
}

pub fn mirror(c: &Crate) -> Report {
    let mut report = Report::default();
    let mut tables = load_tables(&c.tables, &mut report);
    report.watch.push(c.tables.clone());
    let mut rows: Vec<(String, String)> = Vec::new();
    for table in tables.values_mut() {
        for e in &mut table.entries {
            e.index = rows.len();
            rows.push((e.en.clone(), e.ja.clone()));
        }
    }
    let mut m = Mirror { c, tables, report };
    let root = c.dir.join(c.source_root);
    let text = m.file(&root, true, Path::new(c.source_root));
    // The copy's root is include!d, where `#![...]` and `//!` can't be.
    let (text, inner) = blank_inner_attributes(&text);
    let cargo_root = std::fs::read_to_string(c.dir.join(c.cargo_root)).unwrap_or_default();
    let cargo_root_attrs = squash(
        &blank_inner_attributes(&strip_comments(&cargo_root))
            .1
            .join(""),
    );
    for attr in &inner {
        if !cargo_root_attrs.contains(&squash(attr)) {
            m.report.errors.push(format!(
                "{} の `{}` が {} に無い。写して揃える",
                c.source_root,
                attr.lines().next().unwrap_or(""),
                c.cargo_root
            ));
        }
    }
    let mut text = format!("{}{text}\n", c.first_line_prefix);
    for line in &c.extra {
        text.push_str(line);
        text.push('\n');
    }
    m.write(&c.out.join("ja").join(file_name(c.source_root)), &text);
    let (file, declared) = c.side.table_file();
    m.write(&c.out.join("ja").join(file), &table_source(declared, &rows));
    m.finish();
    m.report
}

fn table_source(declared: &str, rows: &[(String, String)]) -> String {
    let mut s = format!(
        "// Written by desktop/ja/mirror.rs from desktop/ja's tables.\n\
         {declared} TABLE: [(&str, &str); {}] = [\n",
        rows.len()
    );
    for (en, ja) in rows {
        let _ = writeln!(s, "    (\"{en}\", \"{ja}\"),");
    }
    s.push_str("];\n");
    s
}

struct Table {
    name: String,
    entries: Vec<Entry>,
    rules: Vec<Rule>,
    made: Vec<Made>,
    tip_macros: Vec<TipMacro>,
    tip_methods: Vec<String>,
    skips: Vec<Skip>,
    visited: bool,
}

struct Made {
    function: String,
    line: usize,
}

struct TipMacro {
    name: String,
    tip_arg: usize,
}

struct Skip {
    en: String,
    used: bool,
}

struct Entry {
    en: String,
    ja: String,
    tip: bool,
    used: bool,
    index: usize,
}

struct Rule {
    find: String,
    with: String,
    reach: Reach,
    line: usize,
}

#[derive(Clone, Copy)]
enum Reach {
    One,
    All,
    Nth { which: usize, of: usize },
}

impl Reach {
    fn written(self) -> String {
        match self {
            Reach::One => "replace:".to_owned(),
            Reach::All => "replace all:".to_owned(),
            Reach::Nth { which, of } => format!("replace {which} of {of}:"),
        }
    }

    fn expected(self) -> String {
        match self {
            Reach::One => "1 か所のはず".to_owned(),
            Reach::All => "1 か所以上のはず".to_owned(),
            Reach::Nth { of, .. } => format!("{of} か所のはず"),
        }
    }

    fn fits(self, count: usize) -> bool {
        match self {
            Reach::One => count == 1,
            Reach::All => count >= 1,
            Reach::Nth { of, .. } => count == of,
        }
    }
}

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
                let name = format!(
                    "desktop/ja/{}",
                    p.strip_prefix(dir.join("..")).unwrap_or(&p).display()
                );
                report.watch.push(p.clone());
                match std::fs::read_to_string(&p)
                    .map_err(|e| format!("{name} を読めない: {e}"))
                    .and_then(|text| parse_table(&name, &text))
                {
                    Ok(t) => {
                        out.insert(src, t);
                    }
                    Err(e) => report.errors.push(e),
                }
            }
        }
    }
    out
}

fn parse_table(name: &str, text: &str) -> Result<Table, String> {
    let mut t = Table {
        name: name.to_owned(),
        entries: Vec::new(),
        rules: Vec::new(),
        made: Vec::new(),
        tip_macros: Vec::new(),
        tip_methods: Vec::new(),
        skips: Vec::new(),
        visited: false,
    };
    let mut pending: Option<(String, Reach, usize)> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let at = || format!("{name}:{}", i + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(head) = replace_head(line) {
            let (reach, code) = head.map_err(|why| format!("{}: {why}", at()))?;
            if pending.is_some() {
                return Err(format!("{}: 前の replace: に with: が無い", at()));
            }
            pending = Some((code.trim().to_owned(), reach, i + 1));
            continue;
        }
        if let Some(code) = line.strip_prefix("with:") {
            let (find, reach, line) = pending
                .take()
                .ok_or_else(|| format!("{}: with: の前に replace: が無い", at()))?;
            t.rules.push(Rule {
                find,
                with: code.trim().to_owned(),
                reach,
                line,
            });
            continue;
        }
        if let Some(f) = line.strip_prefix("made:") {
            t.made.push(Made {
                function: f.trim().to_owned(),
                line: i + 1,
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("tip-macro:") {
            let mut parts = rest.split_whitespace();
            let (Some(mac), Some(Ok(n)), None) = (
                parts.next(),
                parts.next().map(str::parse::<usize>),
                parts.next(),
            ) else {
                return Err(format!("{}: tip-macro: <名前> <何番目> と書く", at()));
            };
            t.tip_macros.push(TipMacro {
                name: mac.trim_end_matches('!').to_owned(),
                tip_arg: n,
            });
            continue;
        }
        if let Some(method) = line.strip_prefix("tip-method:") {
            t.tip_methods.push(method.trim().to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("skip ") {
            let (en, rest) = literal(rest.trim_start())
                .ok_or_else(|| format!("{}: skip の \"…\" が読めない", at()))?;
            reason(rest).ok_or_else(|| format!("{}: skip \"…\" の後に # と理由が要る", at()))?;
            t.skips.push(Skip {
                en: en.to_owned(),
                used: false,
            });
            continue;
        }
        t.entries
            .push(entry(line).map_err(|why| format!("{}: {why}", at()))?);
    }
    if pending.is_some() {
        return Err(format!("{name}: replace: の後に with: が無い"));
    }
    Ok(t)
}

fn entry(line: &str) -> Result<Entry, &'static str> {
    let (tip, rest) = match line.strip_prefix("tip ") {
        Some(r) => (true, r.trim_start()),
        None => (false, line),
    };
    let (en, rest) = literal(rest).ok_or("英語の \"…\" が読めない")?;
    let rest = rest.trim_start().strip_prefix('=').ok_or("= が無い")?;
    let (ja, rest) = literal(rest.trim_start()).ok_or("日本語の \"…\" が読めない")?;
    end_of_line(rest).ok_or("行の終わりに余分な物")?;
    if en.is_empty() || ja.is_empty() {
        return Err("空の文は表に置けない");
    }
    if ja.contains('—') {
        return Err("日本語の UI の文に em dash を使わない");
    }
    Ok(Entry {
        en: en.to_owned(),
        ja: ja.to_owned(),
        tip,
        used: false,
        index: 0,
    })
}

fn replace_head(line: &str) -> Option<Result<(Reach, &str), &'static str>> {
    if let Some(code) = line.strip_prefix("replace all:") {
        return Some(Ok((Reach::All, code)));
    }
    if let Some(code) = line.strip_prefix("replace:") {
        return Some(Ok((Reach::One, code)));
    }
    let rest = line.strip_prefix("replace ")?;
    let nth = rest.split_once(':').and_then(|(head, code)| {
        let mut words = head.split_whitespace();
        match (
            words.next().map(str::parse::<usize>),
            words.next(),
            words.next().map(str::parse::<usize>),
            words.next(),
        ) {
            (Some(Ok(which)), Some("of"), Some(Ok(of)), None)
                if 1 <= which && which <= of && of >= 2 =>
            {
                Some((Reach::Nth { which, of }, code))
            }
            _ => None,
        }
    });
    Some(nth.ok_or(
        "replace <何番目> of <当たる数>: と書く(何番目は 1 から当たる数まで、当たる数は 2 以上)",
    ))
}

fn reason(rest: &str) -> Option<&str> {
    rest.trim()
        .strip_prefix('#')
        .map(str::trim)
        .filter(|why| !why.is_empty())
}

fn end_of_line(rest: &str) -> Option<()> {
    let rest = rest.trim();
    (rest.is_empty() || rest.starts_with('#')).then_some(())
}

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

enum Edit {
    Insert {
        at: usize,
        text: String,
    },
    Replace {
        start: usize,
        end: usize,
        with: String,
    },
}

impl Edit {
    fn insert(at: usize, text: impl Into<String>) -> Edit {
        Edit::Insert {
            at,
            text: text.into(),
        }
    }

    fn replace(start: usize, end: usize, with: impl Into<String>) -> Edit {
        Edit::Replace {
            start,
            end,
            with: with.into(),
        }
    }

    fn start(&self) -> usize {
        match self {
            Edit::Insert { at, .. } => *at,
            Edit::Replace { start, .. } => *start,
        }
    }

    fn end(&self) -> usize {
        match self {
            Edit::Insert { at, .. } => *at,
            Edit::Replace { end, .. } => *end,
        }
    }

    fn new_text(&self) -> &str {
        match self {
            Edit::Insert { text, .. } => text,
            Edit::Replace { with, .. } => with,
        }
    }
}

struct Mirror<'a> {
    c: &'a Crate,
    tables: BTreeMap<PathBuf, Table>,
    report: Report,
}

impl Mirror<'_> {
    fn write(&mut self, path: &Path, text: &str) {
        if let Err(e) = write_if_changed(path, text) {
            self.report.errors.push(e);
        }
    }

    fn file(&mut self, path: &Path, is_root: bool, rel: &Path) -> String {
        self.report.watch.push(path.to_path_buf());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.report
                    .errors
                    .push(format!("{} を読めない: {e}", path.display()));
                String::new()
            }
        };
        let toks = lex(&text);
        let mut edits: Vec<Edit> = Vec::new();
        // A mod-rs file's (the root's, a mod.rs's) modules are beside it,
        // anyone else's in the folder named after it.
        let mod_rs = is_root || path.file_name().is_some_and(|n| n == "mod.rs");
        let own_dir = match (mod_rs, path.parent(), path.file_stem()) {
            (true, Some(p), _) => p.to_path_buf(),
            (false, Some(p), Some(stem)) => p.join(stem),
            _ => PathBuf::new(),
        };
        self.child_mods(&text, &toks, path, &own_dir, &mut edits);
        include_paths(&text, &toks, path, &mut edits);
        let file = rel.display().to_string();
        let mut notes: Vec<Note> = Vec::new();
        if let Some(mut table) = self.tables.remove(rel) {
            table.visited = true;
            self.translate(&text, &toks, &file, &mut table, &mut edits);
            if self.c.side == Side::Gpui {
                let sinks = Sinks::find(&text, &toks, &table);
                notes = sinks.rewrite(&text, &toks, &mut edits);
            }
            self.made(&text, &toks, &table, &mut edits);
            self.tables.insert(rel.to_path_buf(), table);
        }
        if is_root {
            if let Some(call) = &self.c.main_call {
                if !main_call(&text, &toks, call, &mut edits) {
                    self.report
                        .errors
                        .push(format!("{} に `fn main() {{` が無い", self.c.source_root));
                }
            }
        }
        let mut out = apply(&file, &text, edits, &mut self.report.errors);
        if let Some(table) = self.tables.get(rel) {
            let mut rewritten = std::collections::BTreeSet::new();
            out = rules(&out, table, &file, &mut self.report.errors, &mut rewritten);
            notes.retain(|n| !rewritten.contains(&n.let_line));
        }
        for n in notes {
            self.report.notes.push(TipNote {
                file: file.clone(),
                line: n.line,
                why: n.why,
            });
        }
        out
    }

    fn child_mods(
        &mut self,
        text: &str,
        toks: &[Tok],
        path: &Path,
        own_dir: &Path,
        edits: &mut Vec<Edit>,
    ) {
        // In `mod a { mod b; }` written out in a file, b is own_dir/a/b.rs.
        let mut inline: Vec<Option<String>> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            match t.kind {
                Kind::Open('{') => {
                    let name = (i >= 2 && toks[i - 2].is_ident(text, "mod"))
                        .then(|| toks[i - 1].text(text).to_owned());
                    inline.push(name);
                }
                Kind::Close('}') => {
                    inline.pop();
                }
                Kind::Ident
                    if t.text(text) == "mod"
                        && i + 2 < toks.len()
                        && toks[i + 1].kind == Kind::Ident
                        && toks[i + 2].is_punct(text, ";") =>
                {
                    let name = toks[i + 1].text(text);
                    let mut dir = own_dir.to_path_buf();
                    for n in inline.iter().flatten() {
                        dir.push(n);
                    }
                    let start = item_start(text, toks, i);
                    let given = path_attribute(text, toks, start);
                    let file = match &given {
                        // `#[path]` inside a written-out mod is from dir, and
                        // outside one from the file's own folder (Rust's rule).
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
                    let has_children = file.file_name().is_some_and(|n| n != "mod.rs")
                        && file.with_extension("").is_dir();
                    let target = if has_children || self.tables.contains_key(&child_rel) {
                        let body = self.file(&file, false, &child_rel);
                        let dest = self.c.out.join("ja").join(&child_rel);
                        self.write(&dest, &body);
                        dest
                    } else {
                        file
                    };
                    let attr = format!("#[path = {:?}] ", clean(&target).display().to_string());
                    match given {
                        Some(((s, e), _)) => edits.push(Edit::replace(s, e, attr.trim_end())),
                        None => edits.push(Edit::insert(toks[start].start, attr)),
                    }
                    i += 3;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn translate(
        &mut self,
        text: &str,
        toks: &[Tok],
        file: &str,
        table: &mut Table,
        edits: &mut Vec<Edit>,
    ) {
        let index: BTreeMap<String, usize> = table
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.en.clone(), i))
            .collect();
        let (runtime, rows) = (self.c.side.runtime(), self.c.side.table());
        let mut walk = Walk::default();
        for (i, t) in toks.iter().enumerate() {
            walk.step(text, toks, i);
            if t.kind != Kind::Str {
                continue;
            }
            let Some(content) = quoted(t.text(text)) else {
                continue;
            };
            let line = text[..t.start].matches('\n').count() + 1;
            let ctx = walk.context(text, toks, i);
            let skipped = match table.skips.iter_mut().find(|s| s.en == content) {
                Some(skip) => {
                    skip.used = true;
                    true
                }
                None => false,
            };
            let Some(&ei) = index.get(content) else {
                let may_translate = matches!(ctx.treatment, Treatment::MayTranslate);
                if may_translate && !skipped && looks_like_words(content) {
                    self.report.missing.push(Spot {
                        file: file.to_owned(),
                        line,
                        en: content.to_owned(),
                    });
                }
                continue;
            };
            let entry = &mut table.entries[ei];
            entry.used = true;
            let row = entry.index;
            let kept_english_because = match ctx.treatment {
                Treatment::LeaveSilently => continue,
                Treatment::KeepEnglish { why } => Some(why),
                Treatment::MayTranslate => match (entry.tip, ctx.format_head) {
                    (true, Some(_)) => Some("tip (format)".to_owned()),
                    (true, None) => {
                        edits.push(Edit::replace(t.start, t.end, format!("{rows}[{row}].0")));
                        Some("tip".to_owned())
                    }
                    (false, Some((mac_start, open_end))) => {
                        edits.push(Edit::replace(
                            mac_start,
                            open_end,
                            format!("{}(\"{}\", ", self.c.side.format_macro(), entry.ja),
                        ));
                        None
                    }
                    (false, None) => {
                        edits.push(Edit::replace(
                            t.start,
                            t.end,
                            format!("{runtime}::t(&{rows}[{row}])"),
                        ));
                        None
                    }
                },
            };
            let (file, en) = (file.to_owned(), content.to_owned());
            match kept_english_because {
                Some(why) => self.report.kept.push(Kept {
                    file,
                    line,
                    en,
                    why,
                }),
                None => self.report.translated.push(Spot { file, line, en }),
            }
        }
    }

    fn made(&mut self, text: &str, toks: &[Tok], table: &Table, edits: &mut Vec<Edit>) {
        let runtime = self.c.side.runtime();
        for Made { function, line } in &table.made {
            let bodies: Vec<(usize, usize)> = (0..toks.len().saturating_sub(1))
                .filter(|&i| toks[i].is_ident(text, "fn") && toks[i + 1].is_ident(text, function))
                .filter_map(|i| fn_body(toks, i))
                .collect();
            match bodies.as_slice() {
                [(open, close)] => {
                    edits.push(Edit::insert(
                        toks[*open].end,
                        format!(" {runtime}::made({function:?}, || {{"),
                    ));
                    edits.push(Edit::insert(toks[*close].start, "})"));
                }
                _ => self.report.errors.push(format!(
                    "{}:{line}: made: {function} に当たる fn が {} 個(1 個のはず)",
                    table.name,
                    bodies.len()
                )),
            }
        }
    }

    fn finish(&mut self) {
        for t in self.tables.values() {
            let has_code = !t.rules.is_empty() || !t.made.is_empty();
            if !t.visited && has_code {
                self.report.errors.push(format!(
                    "{} のファイルが写しの中に無いので、その replace: と made: が当たらない",
                    t.name
                ));
            }
            for e in t.entries.iter().filter(|e| !e.used) {
                self.report.stale.push(TableOnly {
                    table: t.name.clone(),
                    entry: e.en.clone(),
                });
            }
            for skip in t.skips.iter().filter(|s| !s.used) {
                self.report.stale.push(TableOnly {
                    table: t.name.clone(),
                    entry: format!("skip {}", skip.en),
                });
            }
        }
    }
}

fn rules(
    text: &str,
    table: &Table,
    file: &str,
    errors: &mut Vec<String>,
    rewritten: &mut std::collections::BTreeSet<usize>,
) -> String {
    let mut out = text.to_owned();
    for rule in &table.rules {
        let found: Vec<usize> = out
            .match_indices(rule.find.as_str())
            .map(|(at, _)| at)
            .collect();
        if !rule.reach.fits(found.len()) {
            errors.push(format!(
                "{}:{}: {} が写しの {file} の {} か所に当たった({})。Mike が変えた所に合わせて表を直す: {}",
                table.name,
                rule.line,
                rule.reach.written(),
                found.len(),
                rule.reach.expected(),
                rule.find
            ));
            continue;
        }
        let line_of = |at: usize| out[..at].matches('\n').count() + 1;
        out = match rule.reach {
            Reach::Nth { which, .. } => {
                let at = found[which - 1];
                rewritten.insert(line_of(at));
                format!(
                    "{}{}{}",
                    &out[..at],
                    rule.with,
                    &out[at + rule.find.len()..]
                )
            }
            Reach::One | Reach::All => {
                rewritten.extend(found.iter().map(|&at| line_of(at)));
                out.replace(rule.find.as_str(), &rule.with)
            }
        };
    }
    out
}

struct Sinks {
    calls: Vec<(usize, usize, usize)>,
    idents: Vec<usize>,
}

impl Sinks {
    fn find(text: &str, toks: &[Tok], table: &Table) -> Sinks {
        let mut sinks = Sinks {
            calls: Vec::new(),
            idents: Vec::new(),
        };
        for i in 0..toks.len().saturating_sub(2) {
            let open = i + 2;
            if toks[i].is_punct(text, ".")
                && table
                    .tip_methods
                    .iter()
                    .any(|m| toks[i + 1].is_ident(text, m))
                && toks[open].kind == Kind::Open('(')
            {
                let Some(close) = matching(toks, open) else {
                    continue;
                };
                let args = args(text, toks, open, close);
                if let [(a, b)] = args.as_slice() {
                    sinks.calls.push((i + 1, open, close));
                    if b - a == 1 && toks[*a].kind == Kind::Ident {
                        sinks.idents.push(*a);
                    }
                }
                continue;
            }
            let Some(TipMacro { tip_arg, .. }) = table
                .tip_macros
                .iter()
                .find(|m| toks[i].is_ident(text, &m.name))
            else {
                continue;
            };
            if toks[i + 1].is_punct(text, "!") && toks[open].kind == Kind::Open('(') {
                let Some(close) = matching(toks, open) else {
                    continue;
                };
                if let Some(&(a, b)) = args(text, toks, open, close).get(tip_arg.saturating_sub(1))
                {
                    if b - a == 1 && toks[a].kind == Kind::Ident {
                        sinks.idents.push(a);
                    }
                }
            }
        }
        sinks
    }

    fn rewrite(&self, text: &str, toks: &[Tok], edits: &mut Vec<Edit>) -> Vec<Note> {
        let mut notes = Vec::new();
        for &(tip, open, close) in &self.calls {
            edits.push(Edit::replace(
                toks[tip].start,
                toks[open].end,
                "ja_tip(crate::ja_tip::faces(|| ::std::clone::Clone::clone(&(",
            ));
            edits.push(Edit::insert(toks[close].start, ")))"));
        }
        let mut wrapped: Vec<usize> = Vec::new();
        let depth = depths(toks);
        for &at in &self.idents {
            let line = text[..toks[at].start].matches('\n').count() + 1;
            match self.binding(text, toks, &depth, at) {
                Ok(None) => {}
                Ok(Some(b)) if wrapped.contains(&b.let_at) => {}
                Ok(Some(b)) => {
                    wrapped.push(b.let_at);
                    match b.made_twice {
                        MadeTwice::Element { first, end } => {
                            edits.push(Edit::insert(toks[first].start, "crate::ja_tip::both(|| "));
                            edits.push(Edit::insert(toks[end - 1].end, ")"));
                        }
                        MadeTwice::Value { eq, semi, pair } => {
                            edits.push(Edit::insert(toks[eq].end, " crate::ja_tip::both(||"));
                            let close = match pair {
                                None => ")",
                                Some(PairSide::First) => ").first_is_tooltip()",
                                Some(PairSide::Second) => ").second_is_tooltip()",
                            };
                            edits.push(Edit::insert(toks[semi].start, close));
                        }
                    }
                }
                Err((let_at, why)) => notes.push(Note {
                    line,
                    let_line: text[..toks[let_at].start].matches('\n').count() + 1,
                    why,
                }),
            }
        }
        notes
    }

    fn binding(
        &self,
        text: &str,
        toks: &[Tok],
        depth: &[usize],
        at: usize,
    ) -> Result<Option<Binding>, (usize, String)> {
        let name = toks[at].text(text);
        let mut lowest = depth[at];
        let mut k = at;
        while k > 0 {
            k -= 1;
            lowest = lowest.min(depth[k]);
            if toks[k].is_ident(text, "fn")
                || is_parameter(text, toks, k, name)
                || is_handed_on(text, toks, depth, k, lowest, name)
            {
                return Ok(None);
            }
            if !toks[k].is_ident(text, "let") || depth[k] > lowest {
                continue;
            }
            let Some((tuple, eq)) = pattern(text, toks, k, name) else {
                continue;
            };
            let semi = (eq + 1..toks.len())
                .find(|&j| toks[j].is_punct(text, ";") && depth[j] == depth[k])
                .ok_or_else(|| {
                    (
                        k,
                        format!("ツールチップの `{name}` の let の終わりが見つからない"),
                    )
                })?;
            if is_alias(text, &toks[eq + 1..semi]) {
                return Ok(None);
            }
            if let Some(colon) = (k + 1..eq).find(|&j| toks[j].is_punct(text, ":")) {
                let ty = match tuple {
                    Some((place, _)) => tuple_items(text, toks, colon + 1, eq).get(place).copied(),
                    None => Some((colon + 1, eq)),
                };
                if ty.is_some_and(|(a, b)| is_str_ref(text, &toks[a..b])) {
                    return Ok(None);
                }
                return Err((
                    k,
                    format!(
                        "ツールチップの `{name}` の let に型があるので、二度は作らない(表示は作った時の言語)"
                    ),
                ));
            }
            let made_twice = match tuple {
                Some((place, size)) if size > 2 => {
                    match long_tuple_element(text, toks, eq, semi, place, size) {
                        Some((first, end)) => MadeTwice::Element { first, end },
                        None => {
                            return Err((
                                k,
                                format!(
                                    "ツールチップの `{name}` が 3 つ以上の組の let にあるので、二度は作らない(表示は作った時の言語)"
                                ),
                            ))
                        }
                    }
                }
                Some((0, _)) => MadeTwice::Value {
                    eq,
                    semi,
                    pair: Some(PairSide::First),
                },
                Some(_) => MadeTwice::Value {
                    eq,
                    semi,
                    pair: Some(PairSide::Second),
                },
                None => MadeTwice::Value {
                    eq,
                    semi,
                    pair: None,
                },
            };
            let jumps = ["return", "break", "continue", "await"];
            if toks[eq + 1..semi]
                .iter()
                .any(|t| jumps.iter().any(|j| t.is_ident(text, j)) || t.is_punct(text, "?"))
            {
                return Err((
                    k,
                    format!(
                        "ツールチップの `{name}` を作る式が途中で抜けるので、二度は作らない(表示は作った時の言語)"
                    ),
                ));
            }
            let mut j = semi + 1;
            while j < toks.len() && depth[j] >= depth[k] {
                if toks[j].is_ident(text, "let") && pattern(text, toks, j, name).is_some() {
                    if depth[j] == depth[k] {
                        break;
                    }
                    let inner = depth[j];
                    while j < toks.len() && depth[j] >= inner {
                        j += 1;
                    }
                    continue;
                }
                let used = toks[j].is_ident(text, name)
                    && !(j > 0 && toks[j - 1].is_punct(text, "."))
                    && !self.idents.contains(&j);
                if used {
                    return Err((
                        k,
                        format!(
                            "ツールチップの `{name}` がほかにも使われるので、二度は作らない(表示は作った時の言語)"
                        ),
                    ));
                }
                j += 1;
            }
            return Ok(Some(Binding {
                let_at: k,
                made_twice,
            }));
        }
        Ok(None)
    }
}

fn is_parameter(text: &str, toks: &[Tok], k: usize, name: &str) -> bool {
    let after_let = |j: usize| j > 0 && toks[j - 1].is_ident(text, "let");
    let in_let = after_let(k) || (k > 0 && toks[k - 1].is_ident(text, "mut") && after_let(k - 1));
    toks[k].is_ident(text, name)
        && toks.get(k + 1).is_some_and(|n| n.is_punct(text, ":"))
        && !in_let
}

fn is_handed_on(
    text: &str,
    toks: &[Tok],
    depth: &[usize],
    k: usize,
    lowest: usize,
    name: &str,
) -> bool {
    toks[k].is_ident(text, name)
        && ((closure_param(text, toks, k) && depth[k] <= lowest)
            || (taken_out(text, toks, k) && depth[k] <= lowest + 1))
}

fn is_alias(text: &str, init: &[Tok]) -> bool {
    init.iter()
        .all(|t| t.kind == Kind::Ident || t.is_punct(text, ".") || t.is_punct(text, "::"))
}

fn long_tuple_element(
    text: &str,
    toks: &[Tok],
    eq: usize,
    semi: usize,
    place: usize,
    size: usize,
) -> Option<(usize, usize)> {
    let items = tuple_items(text, toks, eq + 1, semi);
    if items.len() == size {
        items.get(place).copied()
    } else {
        None
    }
}

struct Note {
    line: usize,
    let_line: usize,
    why: String,
}

struct Binding {
    let_at: usize,
    made_twice: MadeTwice,
}

enum MadeTwice {
    Value {
        eq: usize,
        semi: usize,
        pair: Option<PairSide>,
    },
    Element {
        first: usize,
        end: usize,
    },
}

enum PairSide {
    First,
    Second,
}

fn pattern(
    text: &str,
    toks: &[Tok],
    k: usize,
    name: &str,
) -> Option<(Option<(usize, usize)>, usize)> {
    let eq = (k + 1..toks.len()).find(|&j| {
        toks[j].is_punct(text, "=")
            || toks[j].is_punct(text, ";")
            || toks[j].kind == Kind::Open('{')
    })?;
    if !toks[eq].is_punct(text, "=") {
        return None;
    }
    let names: Vec<&str> = toks[k + 1..eq]
        .iter()
        .take_while(|t| !t.is_punct(text, ":"))
        .filter(|t| t.kind == Kind::Ident && !t.is_ident(text, "mut"))
        .map(|t| t.text(text))
        .collect();
    let tuple = toks[k + 1].kind == Kind::Open('(');
    match (tuple, names.as_slice()) {
        (false, [only]) if *only == name => Some((None, eq)),
        (true, all) if all.len() >= 2 => all
            .iter()
            .position(|n| *n == name)
            .map(|place| (Some((place, all.len())), eq)),
        _ => None,
    }
}

fn tuple_items(text: &str, toks: &[Tok], start: usize, end: usize) -> Vec<(usize, usize)> {
    match (start < end).then(|| matching(toks, start)).flatten() {
        Some(close) if toks[start].kind == Kind::Open('(') && close + 1 == end => {
            args(text, toks, start, close)
        }
        _ => Vec::new(),
    }
}

fn is_str_ref(text: &str, ty: &[Tok]) -> bool {
    match ty {
        [amp, s] => amp.is_punct(text, "&") && s.is_ident(text, "str"),
        [amp, life, s] => {
            amp.is_punct(text, "&") && life.kind == Kind::Lifetime && s.is_ident(text, "str")
        }
        _ => false,
    }
}

fn closure_param(text: &str, toks: &[Tok], k: usize) -> bool {
    let bar = |j: usize| toks[j].is_punct(text, "|");
    let between = |j: usize| toks[j].kind == Kind::Ident || toks[j].is_punct(text, ",");
    let before = (0..k).rev().find(|&j| !between(j));
    let after = (k + 1..toks.len()).find(|&j| !between(j));
    match (before, after) {
        // Not `a | name | b`: the bar that opens the parameters follows
        // what can't end an expression (`(`, `,`, `=`, `move`).
        (Some(b), Some(a)) if bar(b) && bar(a) => {
            b == 0 || {
                let p = &toks[b - 1];
                matches!(p.kind, Kind::Open(_))
                    || p.is_punct(text, ",")
                    || p.is_punct(text, "=")
                    || p.is_ident(text, "move")
            }
        }
        _ => false,
    }
}

fn taken_out(text: &str, toks: &[Tok], k: usize) -> bool {
    k >= 2
        && toks[k - 1].kind == Kind::Open('(')
        && toks[k - 2].kind == Kind::Ident
        && toks.get(k + 1).is_some_and(|t| t.kind == Kind::Close(')'))
        && toks.get(k + 2).is_some_and(|t| {
            t.is_punct(text, "=>")
                || t.is_punct(text, "=")
                || t.is_punct(text, "|")
                || t.is_ident(text, "if")
        })
}

fn depths(toks: &[Tok]) -> Vec<usize> {
    let mut d = 0usize;
    toks.iter()
        .map(|t| match t.kind {
            Kind::Open(_) => {
                d += 1;
                d - 1
            }
            Kind::Close(_) => {
                d = d.saturating_sub(1);
                d
            }
            _ => d,
        })
        .collect()
}

fn matching(toks: &[Tok], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, t) in toks.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn args(text: &str, toks: &[Tok], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = open + 1;
    for (j, t) in toks.iter().enumerate().take(close).skip(open + 1) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth = depth.saturating_sub(1),
            Kind::Punct if depth == 0 && t.is_punct(text, ",") => {
                out.push((start, j));
                start = j + 1;
            }
            _ => {}
        }
    }
    if start < close {
        out.push((start, close));
    }
    out
}

fn fn_body(toks: &[Tok], fn_at: usize) -> Option<(usize, usize)> {
    let params = (fn_at..toks.len()).find(|&j| toks[j].kind == Kind::Open('('))?;
    let after = matching(toks, params)?;
    let open = (after..toks.len()).find(|&j| toks[j].kind == Kind::Open('{'))?;
    Some((open, matching(toks, open)?))
}

/// Copies live elsewhere: a relative `include_str!("…")` and its kind get
/// an absolute path.
fn include_paths(text: &str, toks: &[Tok], path: &Path, edits: &mut Vec<Edit>) {
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
                edits.push(Edit::replace(
                    toks[i + 3].start,
                    toks[i + 3].end,
                    format!("{abs:?}"),
                ));
            }
        }
    }
}

fn rel_in_crate(dir: &Path, file: &Path) -> PathBuf {
    let f = clean(file);
    f.strip_prefix(clean(dir))
        .map(Path::to_path_buf)
        .unwrap_or(f)
}

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

fn quoted(lit: &str) -> Option<&str> {
    lit.strip_prefix('"')?.strip_suffix('"')
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn write_if_changed(path: &Path, text: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("{} を作れない: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{} を書けない: {e}", path.display()))
}

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

fn path_attribute(text: &str, toks: &[Tok], start: usize) -> Option<((usize, usize), String)> {
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
        if toks.get(j + 1).is_some_and(|t| t.is_ident(text, "path"))
            && toks.get(j + 3).is_some_and(|t| t.kind == Kind::Str)
        {
            let inner = quoted(toks[j + 3].text(text))?;
            return Some(((toks[j - 1].start, toks[end - 1].end), inner.to_owned()));
        }
        end = j - 1;
    }
    None
}

fn main_call(text: &str, toks: &[Tok], call: &str, edits: &mut Vec<Edit>) -> bool {
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
            edits.push(Edit::insert(toks[i + 4].end, format!(" {call}")));
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

fn apply(file: &str, text: &str, mut edits: Vec<Edit>, errors: &mut Vec<String>) -> String {
    edits.sort_by_key(|e| (e.start(), matches!(e, Edit::Replace { .. }), e.end()));
    let mut out = String::with_capacity(text.len() + edits.len() * 40);
    let mut at = 0;
    for edit in &edits {
        if edit.start() < at {
            let before = &text[..edit.start()];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            errors.push(format!(
                "{file}:{line}:{column}: 写しの差し替え `{}` が、前の差し替えと重なる(desktop/ja/mirror.rs の誤り)",
                edit.new_text()
            ));
            continue;
        }
        out.push_str(&text[at..edit.start()]);
        out.push_str(edit.new_text());
        at = edit.end();
    }
    out.push_str(&text[at..]);
    out
}

fn blank_inner_attributes(text: &str) -> (String, Vec<String>) {
    let toks = lex(text);
    let mut out: Vec<u8> = text.as_bytes().to_vec();
    let mut attrs = Vec::new();
    let mut i = 0;
    // Only the outermost (a `mod x { #![…] }` keeps its own).
    let mut outer = 0i32;
    while i + 2 < toks.len() {
        match toks[i].kind {
            Kind::Open(_) => outer += 1,
            Kind::Close(_) => outer -= 1,
            _ => {}
        }
        if outer == 0
            && toks[i].is_punct(text, "#")
            && toks[i + 1].is_punct(text, "!")
            && toks[i + 2].kind == Kind::Open('[')
        {
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
    let s = String::from_utf8(out).unwrap_or_default();
    let s = s
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("//!") {
                l.replacen("//!", "// ", 1)
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (s, attrs)
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<String>()
}

fn looks_like_words(content: &str) -> bool {
    let mut bare = String::new();
    let mut depth = 0usize;
    for c in content.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => bare.push(c),
            _ => {}
        }
    }
    let content = bare.trim();
    let letters = content.chars().filter(char::is_ascii_alphabetic).count();
    if letters < 2 || content.contains("::") || content.contains('%') {
        return false;
    }
    if content.contains(' ') {
        return true;
    }
    let word = content.trim_end_matches(['.', '…', ':', '?', '!']);
    let mut chars = word.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    first_ok && chars.all(|c| c.is_ascii_lowercase() || c == '\'')
}

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
            // 'a' or '\n' is a char; 'a with no closing quote a lifetime.
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
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == b'_'
                    || (b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)))
            {
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
            let len = if matches!(
                two,
                "=>" | "==" | "!=" | "::" | "->" | "&&" | "||" | "<=" | ">="
            ) {
                2
            } else {
                1
            };
            let len = if !text.is_char_boundary(i + len) {
                1
            } else {
                len
            };
            push(&mut toks, Kind::Punct, i, i + len);
            i += len;
        }
    }
    toks
}

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

pub struct Context {
    pub treatment: Treatment,
    pub format_head: Option<(usize, usize)>,
}

pub enum Treatment {
    LeaveSilently,
    KeepEnglish { why: String },
    MayTranslate,
}

fn in_pattern(text: &str, toks: &[Tok], i: usize) -> bool {
    let wrapped = i >= 3
        && toks[i - 1].kind == Kind::Open('(')
        && matches!(toks[i - 2].text(text), "Some" | "Ok" | "Err");
    if wrapped && matches!(toks[i - 3].text(text), "==" | "!=") {
        return true;
    }
    let mut depth = 0usize;
    let open = (0..i).rev().find(|&j| match toks[j].kind {
        Kind::Close(_) => {
            depth += 1;
            false
        }
        Kind::Open(_) if depth > 0 => {
            depth -= 1;
            false
        }
        Kind::Open(_) => true,
        _ => false,
    });
    let Some(close) = open
        .filter(|&o| toks[o].kind == Kind::Open('('))
        .and_then(|o| matching(toks, o))
    else {
        return false;
    };
    let after = toks[close + 1..]
        .iter()
        .find(|t| t.kind != Kind::Close(')'))
        .map_or("", |t| t.text(text));
    matches!(after, "=>" | "|" | "=")
}

#[derive(Default)]
struct Walk {
    frames: Vec<Frame>,
    test_next: bool,
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
    index: bool,
}

const KEEP: &[&str] = &[
    "aria_label",
    "icon",
    "json",
    "id",
    "key_context",
    "debug",
    "info",
    "warn",
    "error",
    "trace",
    "log",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "expect",
    "println",
    "eprintln",
    "write",
    "writeln",
    "format_args",
    "unreachable",
    "matches",
    "include_str",
    "env",
    "concat",
    "starts_with",
    "ends_with",
    "contains",
    "strip_prefix",
    "strip_suffix",
    "split",
    "rsplit",
    "split_once",
    "rsplit_once",
    "splitn",
    "replace",
    "replacen",
    "find",
    "rfind",
    "trim_start_matches",
    "trim_end_matches",
    "get",
    "eq",
];

const BEFORE_ARRAY: &[&str] = &[
    "in", "return", "else", "match", "if", "mut", "ref", "move", "let",
];

impl Walk {
    fn step(&mut self, text: &str, toks: &[Tok], i: usize) {
        let t = &toks[i];
        match t.kind {
            Kind::Open(open) => self.open(text, toks, i, open),
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
                let next_fn = toks
                    .get(i + 1)
                    .is_some_and(|n| n.is_ident(text, "fn") || n.is_ident(text, "unsafe"));
                let in_braces = self.frames.last().is_none_or(|f| f.open == '{');
                if !next_fn && in_braces && self.const_depth.is_none() {
                    self.const_depth = Some(self.frames.len());
                }
            }
            _ => {}
        }
    }

    fn open(&mut self, text: &str, toks: &[Tok], i: usize, open: char) {
        let t = &toks[i];
        let prev = i.checked_sub(1).map(|p| &toks[p]);
        let (callee, is_macro, macro_start) = match prev {
            Some(p) if p.is_punct(text, "!") && i >= 2 && toks[i - 2].kind == Kind::Ident => (
                Some(toks[i - 2].text(text).to_owned()),
                true,
                toks[i - 2].start,
            ),
            Some(p) if p.kind == Kind::Ident && open == '(' => {
                (Some(p.text(text).to_owned()), false, p.start)
            }
            _ => (None, false, t.start),
        };
        let attr = open == '['
            && prev.is_some_and(|p| {
                p.is_punct(text, "#")
                    || p.is_punct(text, "!") && i >= 2 && toks[i - 2].is_punct(text, "#")
            });
        let index = open == '['
            && prev.is_some_and(|p| {
                (p.kind == Kind::Ident && !BEFORE_ARRAY.contains(&p.text(text)))
                    || matches!(p.kind, Kind::Close(')') | Kind::Close(']'))
            });
        let parent_skip = self.frames.last().is_some_and(|f| f.skip);
        let skip = parent_skip || (open == '{' && self.test_next);
        if open == '{' {
            self.test_next = false;
        }
        if attr {
            let body: String = toks[i + 1..].iter().take(6).map(|t| t.text(text)).collect();
            if body.starts_with("cfg(test)")
                || body.starts_with("test]")
                || body.starts_with("cfg(feature=\"automation\")")
            {
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
            index,
        });
    }

    fn context(&self, text: &str, toks: &[Tok], i: usize) -> Context {
        let prev = i.checked_sub(1).map(|p| toks[p].text(text)).unwrap_or("");
        let next = toks.get(i + 1).map_or("", |t| t.text(text));
        let in_skip = self.frames.iter().any(|f| f.skip || f.attr);
        let compared = matches!(prev, "==" | "!=")
            || matches!(next, "==" | "!=" | "=>" | "|")
            || in_pattern(text, toks, i);
        let left_silently = in_skip || self.const_depth.is_some() || compared;
        let call = self
            .frames
            .iter()
            .rev()
            .find(|f| f.open == '(' || f.is_macro);
        let kept_because = if self.frames.last().is_some_and(|f| f.index) {
            Some("index".to_owned())
        } else {
            call.and_then(|f| f.callee.clone())
                .filter(|c| KEEP.contains(&c.as_str()))
        };
        let treatment = match kept_because {
            _ if left_silently => Treatment::LeaveSilently,
            Some(why) => Treatment::KeepEnglish { why },
            None => Treatment::MayTranslate,
        };
        let format_head = self.frames.last().and_then(|f| {
            (f.is_macro && f.callee.as_deref() == Some("format") && f.arg == 0)
                .then_some((f.macro_start, f.open_end))
        });
        Context {
            treatment,
            format_head,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Copied {
        text: String,
        report: Report,
        table: String,
    }

    fn copy_as(side: Side, src: &str, table: &str) -> Copied {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ja-mirror-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("ja")).unwrap();
        std::fs::create_dir_all(dir.join("tables")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub mod a;\n").unwrap();
        std::fs::write(dir.join("src/a.rs"), src).unwrap();
        std::fs::write(dir.join("ja/lib.rs"), "").unwrap();
        std::fs::write(dir.join("tables/a.txt"), table).unwrap();
        let report = mirror(&Crate {
            dir: dir.clone(),
            source_root: "src/lib.rs",
            cargo_root: "ja/lib.rs",
            tables: dir.join("tables"),
            side,
            first_line_prefix: String::new(),
            extra: vec![],
            main_call: None,
            out: dir.join("out"),
        });
        let text = std::fs::read_to_string(dir.join("out/ja/src/a.rs")).unwrap_or_default();
        let (file, _) = side.table_file();
        let table = std::fs::read_to_string(dir.join("out/ja").join(file)).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        Copied {
            text,
            report,
            table,
        }
    }

    fn copy(src: &str, table: &str) -> Copied {
        copy_as(Side::Core, src, table)
    }

    #[test]
    fn a_literal_on_screen_picks_its_language_from_the_table() {
        let c = copy(
            "fn f() -> &'static str {\n    \"Dismiss\"\n}\n",
            "\"Dismiss\" = \"閉じる\"\n",
        );
        assert_eq!(
            c.text,
            "fn f() -> &'static str {\n    crate::ja::t(&crate::ja::TABLE[0])\n}\n"
        );
        assert!(c.table.contains("pub static TABLE: [(&str, &str); 1] = ["));
        assert!(c.table.contains("(\"Dismiss\", \"閉じる\"),"));
        assert_eq!(c.report.translated.len(), 1);
        assert!(c.report.errors.is_empty(), "{:?}", c.report.errors);
    }

    #[test]
    fn a_format_keeps_its_arguments_and_its_lines() {
        let src = "fn f(t: &str) -> String {\n    format!(\n        \"Open {t}\",\n    )\n}\n";
        let c = copy(src, "\"Open {t}\" = \"{t} を開く\"\n");
        assert_eq!(c.text.lines().count(), src.lines().count());
        assert!(
            c.text
                .contains("ja_format!(\"{t} を開く\", \n        \"Open {t}\","),
            "{}",
            c.text
        );
    }

    #[test]
    fn names_logs_tests_and_comparisons_stay_english() {
        let src = "fn f(e: E) -> E {\n    e.aria_label(\"Copy\").child(\"Copy\")\n}\nfn g(x: &str) -> bool { x == \"Copy\" }\nconst C: &str = \"Copy\";\nfn k(v: V) -> V { v[\"Copy\"] }\nfn m(x: Option<&str>, y: &str) -> u8 { match x { Some(\"Copy\") | None => 1, _ if x == Some(\"Copy\") => 2, _ => match (x, y) { (Some(_), \"Copy\") => 3, _ => 4 } } }\nfn n(p: Option<(&str, u8)>) -> u8 { if let Some((\"Copy\", r)) = p { r } else { 0 } }\nfn o(a: A) -> B { match a { A => b(\"Copy\", 1), _ => c } }\n#[cfg(test)]\nmod tests {\n    fn h() { \"Copy\"; }\n}\n";
        let c = copy(src, "\"Copy\" = \"コピー\"\n");
        assert!(
            c.text
                .contains("e.aria_label(\"Copy\").child(crate::ja::t(&crate::ja::TABLE[0]))"),
            "{}",
            c.text
        );
        assert!(c.text.contains("x == \"Copy\""));
        assert!(c.text.contains("const C: &str = \"Copy\";"));
        assert!(c.text.contains("v[\"Copy\"]"));
        assert!(c.text.contains(
            "Some(\"Copy\") | None => 1, _ if x == Some(\"Copy\") => 2, _ => match (x, y) { (Some(_), \"Copy\") => 3"
        ));
        assert!(c.text.contains("if let Some((\"Copy\", r)) = p"));
        assert!(
            c.text
                .contains("A => b(crate::ja::t(&crate::ja::TABLE[0]), 1)"),
            "{}",
            c.text
        );
        assert!(c.text.contains("fn h() { \"Copy\"; }"));
        assert_eq!(c.report.translated.len(), 2);
        assert_eq!(c.report.kept.len(), 2);
    }

    #[test]
    fn a_tip_entry_is_english_where_written_and_found_by_its_row() {
        let c = copy(
            "fn f(e: E) -> E { e.id(\"Only you\").child(\"Only you\") }\n",
            "tip \"Only you\" = \"自分だけ\"\n",
        );
        assert!(
            c.text
                .contains("e.id(\"Only you\").child(crate::ja::TABLE[0].0)"),
            "{}",
            c.text
        );
    }

    #[test]
    fn new_words_lowercase_words_and_gone_words_are_reported() {
        let c = copy(
            "fn f(e: E) -> E { e.child(\"Brand new words\").child(\"added\").tip(\"A new tooltip\").id(\"header-title\").child(\"threadId\").child(\"Skipped words\") }\n",
            "\"Not there any more\" = \"もう無い\"\nskip \"Skipped words\"  # 鍵\nskip \"Gone too\"  # 鍵\n",
        );
        let missing: Vec<&str> = c.report.missing.iter().map(|m| m.en.as_str()).collect();
        assert_eq!(missing, ["Brand new words", "added", "A new tooltip"]);
        let stale: Vec<&str> = c.report.stale.iter().map(|s| s.entry.as_str()).collect();
        assert_eq!(stale, ["Not there any more", "skip Gone too"]);
    }

    #[test]
    fn a_replace_that_misses_or_matches_twice_stops_the_build() {
        let src = "fn f() { a(); a(); b(); }\n";
        let missed = copy(src, "replace: c();\nwith: d();\n");
        assert_eq!(missed.report.errors.len(), 1, "{:?}", missed.report.errors);
        assert!(missed.report.errors[0].contains("tables/a.txt:1"));
        assert!(missed.report.errors[0].contains("0 か所"));
        let twice = copy(src, "\n\nreplace: a();\nwith: d();\n");
        assert!(twice.report.errors[0].contains("tables/a.txt:3"));
        assert!(twice.report.errors[0].contains("2 か所"));
        let once = copy(src, "replace: b();\nwith: d();\n");
        assert!(once.report.errors.is_empty());
        assert!(once.text.contains("a(); a(); d();"));
        let all = copy(src, "replace all: a();\nwith: d();\n");
        assert!(all.report.errors.is_empty());
        assert!(all.text.contains("d(); d(); b();"));
    }

    #[test]
    fn a_replace_can_name_which_of_a_known_number_of_matches() {
        let src = "fn f() { a(); a(); b(); }\n";
        let second = copy(src, "replace 2 of 2: a();\nwith: d();\n");
        assert!(
            second.report.errors.is_empty(),
            "{:?}",
            second.report.errors
        );
        assert!(second.text.contains("a(); d(); b();"), "{}", second.text);
        let first = copy(src, "replace 1 of 2: a();\nwith: d();\n");
        assert!(first.text.contains("d(); a(); b();"), "{}", first.text);
        let more = copy(src, "\nreplace 1 of 3: a();\nwith: d();\n");
        assert_eq!(more.report.errors.len(), 1, "{:?}", more.report.errors);
        assert!(more.report.errors[0].contains("tables/a.txt:2"));
        assert!(more.report.errors[0].contains("2 か所"));
        assert!(parse_table("t", "replace 3 of 2: a();\nwith: d();\n").is_err());
        assert!(parse_table("t", "replace 0 of 2: a();\nwith: d();\n").is_err());
        assert!(parse_table("t", "replace 1 of 1: a();\nwith: d();\n").is_err());
    }

    #[test]
    fn a_skip_without_a_reason_stops_the_build_at_its_line() {
        let src = "fn f(e: E) -> E { e.child(\"Some words\") }\n";
        let bare = copy(src, "# 頭\nskip \"Some words\"\n");
        assert_eq!(bare.report.errors.len(), 1, "{:?}", bare.report.errors);
        assert!(
            bare.report.errors[0].contains("tables/a.txt:2"),
            "{:?}",
            bare.report.errors
        );
        let blank = copy(src, "skip \"Some words\"  #  \n");
        assert_eq!(blank.report.errors.len(), 1, "{:?}", blank.report.errors);
        assert!(blank.report.errors[0].contains("tables/a.txt:1"));
        let given = copy(src, "skip \"Some words\"  # 鍵\n");
        assert!(given.report.errors.is_empty(), "{:?}", given.report.errors);
        assert!(given.report.missing.is_empty());
    }

    #[test]
    fn overlapping_edits_stop_the_build_at_their_place() {
        let text = "ab\ncdef\n";
        let mut errors = Vec::new();
        apply(
            "src/a.rs",
            text,
            vec![
                Edit::replace(3, 6, "X"),
                Edit::replace(4, 5, "Y"),
                Edit::insert(5, "Z"),
            ],
            &mut errors,
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors.iter().all(|e| e.contains("src/a.rs:2")),
            "{errors:?}"
        );
        let mut errors = Vec::new();
        let out = apply(
            "src/a.rs",
            text,
            vec![
                Edit::insert(5, ">"),
                Edit::replace(3, 5, "X"),
                Edit::insert(3, "<"),
                Edit::insert(5, "!"),
            ],
            &mut errors,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(out, "ab\n<X>!ef\n");
    }

    #[test]
    fn a_copy_that_cant_be_written_stops_the_build() {
        let mut m = Mirror {
            c: &Crate {
                dir: PathBuf::new(),
                source_root: "src/lib.rs",
                cargo_root: "ja/lib.rs",
                tables: PathBuf::new(),
                side: Side::Core,
                first_line_prefix: String::new(),
                extra: vec![],
                main_call: None,
                out: PathBuf::new(),
            },
            tables: BTreeMap::new(),
            report: Report::default(),
        };
        m.write(&std::env::temp_dir(), "x");
        assert_eq!(m.report.errors.len(), 1);
    }

    #[test]
    fn a_tooltip_call_is_made_twice_and_its_variable_too() {
        let src = "fn f(e: E, p: &str) -> E {\n    let tip = format!(\"In {p}\");\n    e.tip(tip).child(\"x\")\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-method: tip\n\"In {p}\" = \"{p} の中\"\n",
        );
        assert_eq!(c.text.lines().count(), src.lines().count());
        assert!(
            c.text.contains("let tip = crate::ja_tip::both(|| threadbear_core::ja_format!(\"{p} の中\", \"In {p}\"));"),
            "{}",
            c.text
        );
        assert!(
            c.text.contains(
                "e.ja_tip(crate::ja_tip::faces(|| ::std::clone::Clone::clone(&(tip)))).child"
            ),
            "{}",
            c.text
        );
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
    }

    #[test]
    fn a_tooltip_macro_s_variable_from_a_pair_is_made_twice() {
        let src = "fn f(e: E) -> E {\n    let (text, tip) = match x { _ => (format!(\"A {x}\"), format!(\"B {x}\")) };\n    tipped!(e, tip).child(text)\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-macro: tipped! 2\n\"A {x}\" = \"甲 {x}\"\n\"B {x}\" = \"乙 {x}\"\n",
        );
        assert!(
            c.text
                .contains("let (text, tip) = crate::ja_tip::both(|| match"),
            "{}",
            c.text
        );
        assert!(c.text.contains(").second_is_tooltip();"), "{}", c.text);
        let first = copy_as(
            Side::Gpui,
            &src.replace("let (text, tip)", "let (tip, text)"),
            "tip-macro: tipped! 2\n\"A {x}\" = \"甲 {x}\"\n\"B {x}\" = \"乙 {x}\"\n",
        );
        assert!(
            first.text.contains(").first_is_tooltip();"),
            "{}",
            first.text
        );
    }

    #[test]
    fn a_tooltip_variable_used_elsewhere_or_left_early_is_noted_not_wrapped() {
        let src = "fn f(e: E) -> E {\n    let tip = format!(\"In {p}\");\n    log(&tip);\n    let other = x?;\n    tipped!(e, tip).tip(other)\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-macro: tipped 2\ntip-method: tip\n\"In {p}\" = \"{p} の中\"\n",
        );
        assert!(!c.text.contains("crate::ja_tip::both"), "{}", c.text);
        assert_eq!(c.report.notes.len(), 2, "{:?}", c.report.notes);
        assert!(c.report.notes[0].why.contains("ほかにも使われる"));
        assert!(c.report.notes[1].why.contains("途中で抜ける"));
    }

    #[test]
    fn a_let_of_the_same_name_inside_doesnt_count_as_a_use() {
        let src = "fn f(e: E) -> E {\n    let tip = format!(\"In {p}\");\n    tipped!(e, tip).when(|s| { let tip = format!(\"In {p}\"); tipped!(s, tip) })\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-macro: tipped! 2\n\"In {p}\" = \"{p} の中\"\n",
        );
        assert_eq!(
            c.text.matches("crate::ja_tip::both(||").count(),
            2,
            "{}",
            c.text
        );
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
    }

    #[test]
    fn a_tooltip_parameter_or_handed_on_value_is_not_made_twice() {
        let src = "fn f(e: E, tip: &'static str, s: S) -> E {\n    let tip_text = 1;\n    let other = s.words;\n    e.tip(tip).tip(other)\n}\n";
        let c = copy_as(Side::Gpui, src, "tip-method: tip\n");
        assert!(c.text.contains("e.ja_tip("), "{}", c.text);
        assert!(!c.text.contains("both"), "{}", c.text);
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
    }

    #[test]
    fn a_tooltip_let_with_a_type_is_noted_not_taken_for_a_parameter() {
        let src = "fn f(e: E, p: &str) -> E {\n    let tip: String = format!(\"In {p}\");\n    let mut other: String = format!(\"In {p}\");\n    e.tip(tip).tip(other)\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-method: tip\n\"In {p}\" = \"{p} の中\"\n",
        );
        assert!(!c.text.contains("crate::ja_tip::both"), "{}", c.text);
        assert_eq!(c.report.notes.len(), 2, "{:?}", c.report.notes);
        assert!(
            c.report.notes.iter().all(|n| n.why.contains("型がある")),
            "{:?}",
            c.report.notes
        );
    }

    #[test]
    fn a_closure_s_or_a_pattern_s_tooltip_is_handed_on_not_an_earlier_let() {
        let src = "fn f(e: E, j: Option<String>, k: Option<String>) -> E {\n    let tip = format!(\"In {p}\");\n    let e = e.aria_label(tip.clone()).tip(tip);\n    let e = e.when_some(j, |s, tip| s.tip(tip));\n    match k {\n        Some(tip) if ok => e.tip(tip),\n        Some(tip) => e.tip(tip),\n        None => e,\n    }\n}\n";
        let c = copy_as(
            Side::Gpui,
            src,
            "tip-method: tip\n\"In {p}\" = \"{p} の中\"\n",
        );
        let lines: Vec<usize> = c.report.notes.iter().map(|n| n.line).collect();
        assert_eq!(lines, [3], "{:?}", c.report.notes);
    }

    #[test]
    fn a_tooltip_from_a_longer_tuple_has_its_element_made_twice() {
        let table = "tip-macro: tipped! 2\n\"In {p}\" = \"{p} の中\"\n";
        let src = "fn f(e: E) -> E {\n    let tip = format!(\"In {p}\");\n    let (a, tip, b) = (1, format!(\"In {p}\"), 2);\n    tipped!(e, tip)\n}\n";
        let c = copy_as(Side::Gpui, src, table);
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
        assert!(
            c.text.contains("let (a, tip, b) = (1, crate::ja_tip::both(|| threadbear_core::ja_format!(\"{p} の中\", \"In {p}\")), 2);"),
            "{}",
            c.text
        );
        let src = "fn f(e: E) -> E {\n    let (a, tip, b) = match x { _ => (1, format!(\"In {p}\"), 2) };\n    tipped!(e, tip)\n}\n";
        let c = copy_as(Side::Gpui, src, table);
        assert_eq!(c.report.notes.len(), 1, "{:?}", c.report.notes);
        assert!(
            c.report.notes[0].why.contains("3 つ以上"),
            "{:?}",
            c.report.notes
        );
        let src = "fn f(e: E) -> E {\n    let (a, tip, b): (u8, &str, u8) = match x { _ => (1, \"In\", 2) };\n    let one: &'static str = \"In\";\n    tipped!(e, tip).tip(one)\n}\n";
        let c = copy_as(Side::Gpui, src, &format!("{table}tip-method: tip\n"));
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
    }

    #[test]
    fn a_let_the_table_rewrites_itself_isnt_noted() {
        let src = "fn f(e: E, p: &str) -> E {\n    let tip: String = format!(\"In {p}\");\n    e.tip(tip)\n}\n";
        let table = "tip-method: tip\n\"In {p}\" = \"{p} の中\"\n";
        assert_eq!(copy_as(Side::Gpui, src, table).report.notes.len(), 1);
        let c = copy_as(
            Side::Gpui,
            src,
            &format!("{table}replace: let tip: String =\nwith: let tip = crate::ja_tip::both(|| -> String {{ 0 }}); let _: String =\n"),
        );
        assert!(c.report.notes.is_empty(), "{:?}", c.report.notes);
        assert!(c.report.errors.is_empty(), "{:?}", c.report.errors);
    }

    #[test]
    fn another_type_s_tip_method_is_left_alone() {
        let src = "fn f(row: Row) -> Row {\n    row.tip(\"Some words\")\n}\n";
        let c = copy_as(Side::Gpui, src, "\"Some words\" = \"言葉\"\n");
        assert!(
            c.text
                .contains("row.tip(threadbear_core::ja::t(&crate::ja_table::TABLE[0]))"),
            "{}",
            c.text
        );
    }

    #[test]
    fn a_made_function_runs_its_body_twice() {
        let src = "pub fn icon_tooltip(a: &A) -> String {\n    a.lines().join(\"\\n\")\n}\n";
        let c = copy(src, "made: icon_tooltip\n");
        assert_eq!(
            c.text,
            "pub fn icon_tooltip(a: &A) -> String { crate::ja::made(\"icon_tooltip\", || {\n    a.lines().join(\"\\n\")\n})}\n"
        );
        let missing = copy(src, "made: gone\n");
        assert!(missing.report.errors[0].contains("made: gone"));
    }

    #[test]
    fn a_table_rejects_an_em_dash_and_an_empty_entry() {
        assert!(parse_table("t", "\"A\" = \"あ—い\"\n").is_err());
        assert!(parse_table("t", "\"A\" = \"\"\n").is_err());
        assert!(parse_table("t", "replace: x\nreplace: y\nwith: z\n").is_err());
    }

    #[test]
    fn words_on_the_screen_are_told_from_keys() {
        for words in [
            "Archived",
            "added",
            "Couldn't reach it.",
            "Sign in…",
            "a quiet minute",
        ] {
            assert!(looks_like_words(words), "{words}");
        }
        for key in [
            "threadId",
            "header-title",
            "thread_id",
            "/x/claude",
            "%-d %b",
            "a",
            "{}",
            "v1",
            "{name} {v}",
            " [{button}]",
        ] {
            assert!(!looks_like_words(key), "{key}");
        }
    }
}
