// nexus_scan: the Finding module of GPTNexus (DGK x ULTRA protocol).
// Rust stdlib-only scanner for Python code vulnerabilities.
//
// Rules (v1.0 scope, honestly declared):
//   R001 eval            critical   eval( with word boundary (not reval()
//   R002 exec            critical   exec(
//   R003 dyn_import      critical   __import__(
//   R004 sql_concat      high       SELECT/INSERT/UPDATE/DELETE + string concat or f-string
//   R005 os_system       critical   os.system(
//   R006 shell_true      high       shell=True in subprocess/os calls
//   R007 pickle_loads    high       pickle.loads(
//   R008 hard_secret     medium     password=/api_key=/token= with quoted literal
//
// Output: JSON on stdout. Exit 0 = scan done (parse JSON for findings).
// This is NOT an SEC-bench-Pro-88.2% scanner — it is a rule engine whose
// catch/no-false-positive behavior is proven on the fixture bench (N02-N05).

use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

const VERSION: &str = "1.0.0";

struct Rule {
    id: &'static str,
    rule: &'static str,
    severity: &'static str,
    description: &'static str,
}

const RULES_EVAL: Rule = Rule { id: "R001", rule: "eval", severity: "critical",
    description: "eval() executes arbitrary code — replace with a whitelisted AST evaluator" };
const RULES_EXEC: Rule = Rule { id: "R002", rule: "exec", severity: "critical",
    description: "exec() executes arbitrary statements" };
const RULES_DYN_IMPORT: Rule = Rule { id: "R003", rule: "dyn_import", severity: "critical",
    description: "__import__() loads arbitrary modules dynamically" };
const RULES_SQL_CONCAT: Rule = Rule { id: "R004", rule: "sql_concat", severity: "high",
    description: "SQL built by string concatenation/f-string — injection risk; parameterize" };
const RULES_OS_SYSTEM: Rule = Rule { id: "R005", rule: "os_system", severity: "critical",
    description: "os.system() runs a shell command" };
const RULES_SHELL_TRUE: Rule = Rule { id: "R006", rule: "shell_true", severity: "high",
    description: "subprocess with shell=True exposes shell injection" };
const RULES_PICKLE: Rule = Rule { id: "R007", rule: "pickle_loads", severity: "high",
    description: "pickle.loads() can execute arbitrary code on crafted payloads" };
const RULES_SECRET: Rule = Rule { id: "R008", rule: "hard_secret", severity: "medium",
    description: "hardcoded secret literal — move to environment variable" };

// find "word(" with a word boundary before it (avoids reval(, my_eval( etc.)
fn has_boundary_call(line: &str, word: &str) -> bool {
    let target = format!("{}(", word);
    let bytes = line.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = line[from..].find(&target) {
        let abs = from + rel;
        let ok_before = abs == 0 || {
            let prev = bytes[abs - 1];
            !(prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'.')
        };
        // also skip if inside a comment (crude: line starts with #)
        if ok_before && !line.trim_start().starts_with('#') {
            return true;
        }
        from = abs + 1;
    }
    false
}

fn is_sql_concat(line: &str) -> bool {
    let up = line.to_uppercase();
    let has_kw = up.contains("SELECT ") || up.contains("INSERT ")
        || up.contains("UPDATE ") || up.contains("DELETE FROM");
    if !has_kw {
        return false;
    }
    // concatenation: "..." + expr, or f-string interpolation
    let has_concat = line.contains("\" +") || line.contains("+ \"")
        || line.contains("' +") || line.contains("+ '")
        || line.contains("f\"") || line.contains("f'");
    has_concat
}

fn is_hard_secret(line: &str) -> bool {
    let lower = line.to_lowercase();
    let keys = ["password", "passwd", "api_key", "apikey", "secret", "token"];
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    for k in keys {
        // pattern: key = "literal"   (assignment, not comparison ==)
        let pat_assign = format!("{} =", k);
        let pat_eq = format!("{}=", k);
        let assign_hit = lower.contains(&pat_assign) || lower.contains(&pat_eq);
        if !assign_hit {
            continue;
        }
        // must have a quoted literal value, and not come from env
        let has_quote = line.contains('"') || line.contains('\'');
        let from_env = lower.contains("os.environ") || lower.contains("getenv")
            || lower.contains("environ[");
        // reject empty literal like key = ""
        let empty_lit = lower.contains("\"\"") || lower.contains("''");
        if has_quote && !from_env && !empty_lit {
            return true;
        }
    }
    false
}

fn scan_line(line: &str) -> Vec<&'static Rule> {
    let mut hits: Vec<&'static Rule> = Vec::new();
    // [v1.0.1] ULTRA fix, caught by bench N13: full-line comments are not
    // executable code — a comment that MENTIONS eval(/__import__(/os.system(
    // is documentation, not a vulnerability. A scanner that cries wolf on
    // comments is useless (same class as the T13 false-positive, Task ID 8).
    // Trailing comments are still scanned (conservative: over-detection is
    // safer than under-detection for partial-line comments).
    if line.trim_start().starts_with('#') {
        return hits;
    }
    if has_boundary_call(line, "eval") {
        hits.push(&RULES_EVAL);
    }
    if has_boundary_call(line, "exec") {
        hits.push(&RULES_EXEC);
    }
    if line.contains("__import__(") {
        hits.push(&RULES_DYN_IMPORT);
    }
    if is_sql_concat(line) {
        hits.push(&RULES_SQL_CONCAT);
    }
    if line.contains("os.system(") {
        hits.push(&RULES_OS_SYSTEM);
    }
    if line.contains("shell=True") {
        hits.push(&RULES_SHELL_TRUE);
    }
    if line.contains("pickle.loads(") {
        hits.push(&RULES_PICKLE);
    }
    if is_hard_secret(line) {
        hits.push(&RULES_SECRET);
    }
    hits
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn scan_file(path: &str, findings: &mut Vec<String>) -> usize {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nexus_scan: cannot read {}: {}", path, e);
            return 0;
        }
    };
    let mut count = 0usize;
    for (idx, line) in content.lines().enumerate() {
        for rule in scan_line(line) {
            let snippet = line.trim();
            if snippet.len() > 100 {
                // keep snippet bounded
                let s: String = snippet.chars().take(97).collect();
                findings.push(format!(
                    "{{\"file\":\"{}\",\"line\":{},\"id\":\"{}\",\"rule\":\"{}\",\"severity\":\"{}\",\"description\":\"{}\",\"snippet\":\"{}...\"}}",
                    json_escape(path), idx + 1, rule.id, rule.rule, rule.severity,
                    json_escape(rule.description), json_escape(&s)
                ));
            } else {
                findings.push(format!(
                    "{{\"file\":\"{}\",\"line\":{},\"id\":\"{}\",\"rule\":\"{}\",\"severity\":\"{}\",\"description\":\"{}\",\"snippet\":\"{}\"}}",
                    json_escape(path), idx + 1, rule.id, rule.rule, rule.severity,
                    json_escape(rule.description), json_escape(snippet)
                ));
            }
            count += 1;
        }
    }
    count
}

fn collect_py_files(path: &str, out: &mut Vec<String>) {
    let p = Path::new(path);
    if p.is_file() {
        out.push(path.to_string());
        return;
    }
    if p.is_dir() {
        if let Ok(entries) = fs::read_dir(p) {
            let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            dirs.sort_by_key(|e| e.path());
            for e in dirs {
                let sub = e.path();
                let sub_str = sub.to_string_lossy().to_string();
                if sub.is_dir() {
                    // skip hidden dirs like .nexus, .git, target
                    let name = sub.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" {
                        continue;
                    }
                    collect_py_files(&sub_str, out);
                } else if sub_str.ends_with(".py") {
                    out.push(sub_str);
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: nexus_scan <path>...   (files or directories)");
        exit(2);
    }
    if args[1] == "--version" {
        println!("nexus_scan v{} (GPTNexus Finding module, DGK x ULTRA)", VERSION);
        exit(0);
    }

    let mut files: Vec<String> = Vec::new();
    for path in &args[1..] {
        collect_py_files(path, &mut files);
    }
    if files.is_empty() {
        println!("{{\"scanner\":\"nexus_scan v{}\",\"files_scanned\":0,\"findings\":[],\"count\":0}}", VERSION);
        exit(0);
    }

    let mut findings: Vec<String> = Vec::new();
    let files_scanned = files.len();
    for f in &files {
        scan_file(f, &mut findings);
    }

    let body = findings.join(",");
    println!(
        "{{\"scanner\":\"nexus_scan v{}\",\"files_scanned\":{},\"findings\":[{}],\"count\":{}}}",
        VERSION, files_scanned, body, findings.len()
    );
}
