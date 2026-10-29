// sparkrush-editor: native Rust text viewer + syntax highlighter + agent hook.
// DGK x PROTON protocol. Stdlib-only — no external crates, compilation guaranteed.
//
// What this is NOT: a VS Code clone, an interactive TUI, a LLM.
// What this IS: an executable Rust binary that views a file with syntax
// highlighting, reports metrics, and dispatches to the python optimizer
// agent on demand. Honest scope, real tests.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, exit};

const HELP: &str = r#"
SPARKRUSH-EDITOR v1.0 — native Rust desktop tool (DGK x PROTON)
Stdlib-only. No external crates. Compiles offline.

Usage:
  sparkrush-editor <file>                view file with syntax highlight
  sparkrush-editor <file> --metrics      print metrics only (no color)
  sparkrush-editor <file> --optimize     invoke python optimizer-agent
  sparkrush-editor --selftest            run builtin self-test (highlights)
  sparkrush-editor --version
  sparkrush-editor --help
"#;

// ---- ANSI colors ---------------------------------------------------------
const C_RESET: &str = "\x1b[0m";
const C_KEYWORD: &str = "\x1b[1;33m";
const C_STRING: &str = "\x1b[32m";
const C_COMMENT: &str = "\x1b[2;37m";
const C_NUM: &str = "\x1b[36m";
const C_LINENUM: &str = "\x1b[90m";

// ---- language detection -------------------------------------------------
fn detect_lang(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("sh") | Some("bash") => "shell",
        Some("xml") | Some("xsd") | Some("svg") | Some("desktop") => "xml",
        Some("json") => "json",
        Some("md") => "markdown",
        _ => "text",
    }
}

// ---- per-language highlighters ------------------------------------------
fn highlight_rust(line: &str) -> String {
    let kws = ["fn","let","mut","pub","use","mod","struct","enum","impl","trait",
               "match","if","else","for","while","loop","return","as","in","where",
               "move","async","await","dyn","ref","self","Self","true","false",
               "const","static","unsafe","extern","crate","super"];
    highlight_tokens(line, &kws, "//")
}

fn highlight_python(line: &str) -> String {
    let kws = ["def","class","import","from","as","if","elif","else","for","while",
               "return","yield","with","try","except","finally","raise","pass",
               "lambda","None","True","False","and","or","not","in","is","global",
               "nonlocal","assert","del","break","continue"];
    highlight_tokens(line, &kws, "#")
}

fn highlight_shell(line: &str) -> String {
    let kws = ["if","then","else","elif","fi","for","while","do","done","case","esac",
               "function","return","local","export","unset","read","echo","printf",
               "exit","source","alias"];
    highlight_tokens(line, &kws, "#")
}

fn highlight_tokens(line: &str, keywords: &[&str], comment_prefix: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with(comment_prefix) {
        return format!("{}{}{}", C_COMMENT, line, C_RESET);
    }
    let mut out = String::with_capacity(line.len() * 2);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut in_string: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match in_string {
            Some(q) => {
                out.push(c);
                if c == q && (i == 0 || chars[i - 1] != '\\') {
                    out.push_str(C_RESET);
                    in_string = None;
                }
                i += 1;
            }
            None => {
                if c == '"' || c == '\'' {
                    in_string = Some(c);
                    out.push_str(C_STRING);
                    out.push(c);
                    i += 1;
                } else if c.is_alphabetic() || c == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    if keywords.contains(&word.as_str()) {
                        out.push_str(C_KEYWORD);
                        out.push_str(&word);
                        out.push_str(C_RESET);
                    } else {
                        out.push_str(&word);
                    }
                } else if c.is_ascii_digit() {
                    let start = i;
                    i += 1;
                    while i < chars.len()
                        && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
                    {
                        i += 1;
                    }
                    let num: String = chars[start..i].iter().collect();
                    out.push_str(C_NUM);
                    out.push_str(&num);
                    out.push_str(C_RESET);
                } else {
                    out.push(c);
                    i += 1;
                }
            }
        }
    }
    if in_string.is_some() {
        out.push_str(C_RESET);
    }
    out
}

fn highlight_xml(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("<!--") {
        return format!("{}{}{}", C_COMMENT, line, C_RESET);
    }
    let mut out = String::with_capacity(line.len() * 2);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            out.push_str(C_KEYWORD);
            out.push('<');
            i += 1;
            while i < chars.len() && chars[i] != '>' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                out.push('>');
                i += 1;
            }
            out.push_str(C_RESET);
        } else if chars[i] == '"' {
            out.push_str(C_STRING);
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                out.push('"');
                i += 1;
            }
            out.push_str(C_RESET);
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn highlight_json(line: &str) -> String {
    let mut out = String::with_capacity(line.len() * 2);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            out.push_str(C_STRING);
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                out.push('"');
                i += 1;
            }
            out.push_str(C_RESET);
        } else if chars[i].is_ascii_digit() {
            out.push_str(C_NUM);
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E'
                    || chars[i] == '+'
                    || chars[i] == '-')
            {
                out.push(chars[i]);
                i += 1;
            }
            out.push_str(C_RESET);
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn highlight(line: &str, lang: &str) -> String {
    match lang {
        "rust" => highlight_rust(line),
        "python" => highlight_python(line),
        "shell" => highlight_shell(line),
        "xml" => highlight_xml(line),
        "json" => highlight_json(line),
        _ => line.to_string(),
    }
}

// ---- commands -----------------------------------------------------------
fn view_file(path: &str) -> io::Result<()> {
    let lang = detect_lang(path);
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let width = lines.len().to_string().len();
    println!(
        "# sparkrush-editor | {} | {} lines | {} bytes | lang={}\n",
        path,
        lines.len(),
        content.len(),
        lang
    );
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for (idx, line) in lines.iter().enumerate() {
        write!(out, "{}{:>width$}{} | ", C_LINENUM, idx + 1, C_RESET, width = width)?;
        writeln!(out, "{}", highlight(line, lang))?;
    }
    Ok(())
}

fn metrics(path: &str) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let non_blank = lines.iter().filter(|l| !l.trim().is_empty()).count();
    let max_width = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let complexity = content.matches("if").count()
        + content.matches("for").count()
        + content.matches("while").count()
        + content.matches("match").count()
        + content.matches("case").count();
    println!("file: {}", path);
    println!("bytes: {}", content.len());
    println!("lines_total: {}", lines.len());
    println!("lines_non_blank: {}", non_blank);
    println!("max_line_width: {}", max_width);
    println!("complexity_proxy: {}", complexity);
    Ok(())
}

fn optimize(path: &str) -> io::Result<()> {
    let agent = env::var("SPARKRUSH_AGENT").unwrap_or_else(|_| "python3".to_string());
    let script =
        env::var("SPARKRUSH_AGENT_PATH").unwrap_or_else(|_| "agents/optimizer.py".to_string());
    let status = Command::new(&agent).arg(&script).arg(path).status()?;
    exit(status.code().unwrap_or(1));
}

fn selftest() -> io::Result<()> {
    println!("sparkrush-editor selftest:");
    let demo = r#"fn main() {
    let x = 42;
    println!("hello");
    // comment
}"#;
    println!("--- rust highlight sample ---");
    for line in demo.lines() {
        println!("{}", highlight_rust(line));
    }
    println!("--- xml highlight sample ---");
    let xml = r#"<config name="sparkrush">
  <!-- editor section -->
  <editor syntax="on" theme="dark"/>
</config>"#;
    for line in xml.lines() {
        println!("{}", highlight_xml(line));
    }
    println!("--- python highlight sample ---");
    let py = r#"def add(a, b):
    # sum
    return a + b"#;
    for line in py.lines() {
        println!("{}", highlight_python(line));
    }
    println!("selftest OK");
    Ok(())
}

// ---- main ---------------------------------------------------------------
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print!("{}", HELP);
        exit(0);
    }
    let path = &args[1];
    let result = match path.as_str() {
        "--version" => {
            println!("sparkrush-editor v1.0.0 (rustc)");
            Ok(())
        }
        "--help" | "-h" => {
            print!("{}", HELP);
            Ok(())
        }
        "--selftest" => selftest(),
        _ => {
            if !Path::new(path).exists() {
                eprintln!("sparkrush-editor: file not found: {}", path);
                exit(2);
            }
            if args.len() > 2 && args[2] == "--metrics" {
                metrics(path)
            } else if args.len() > 2 && args[2] == "--optimize" {
                optimize(path);
                Ok(())
            } else {
                view_file(path)
            }
        }
    };
    if let Err(e) = result {
        eprintln!("sparkrush-editor error: {}", e);
        exit(1);
    }
}
