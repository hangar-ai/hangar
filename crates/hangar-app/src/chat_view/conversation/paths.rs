//! The files Claude names in its messages, made links to them.
//!
//! Claude Code names a file as `src/search/index.ts`, or with its line as
//! `src/search/index.ts:42`, in inline code, and sometimes as a Markdown link
//! to a relative path. Before a message is parsed, each such name that is a
//! file of the workspace (checked on disk, relative to its folder) becomes a
//! link of Hangar's own scheme, which carries the workspace: a click opens the
//! file in the workspace's preview, beside the conversation. Anything else is
//! left as written: a name that is not a file there, code blocks, and the
//! links to the web.

use std::path::Path;

/// The scheme of a link to a file of a workspace:
/// `hangar-file:<workspace>|<path>[:<line>]`.
pub const SCHEME: &str = "hangar-file:";

/// The longest name read as a path.
const PATH_MAX: usize = 300;
/// The most names one message has checked on disk.
const CHECKS_MAX: usize = 64;

/// A file a link names: its workspace, its path as written (relative to the
/// workspace's folder, or absolute) and the line named with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLink {
    pub ws: String,
    pub path: String,
    pub line: Option<u32>,
}

/// The file a `hangar-file:` link names.
pub fn file_of(url: &str) -> Option<FileLink> {
    let rest = url.strip_prefix(SCHEME)?;
    let (ws, named) = rest.split_once('|')?;
    let (path, line) = split_line(named);
    (!ws.is_empty() && !path.is_empty()).then(|| FileLink {
        ws: ws.to_string(),
        path: path.to_string(),
        line,
    })
}

/// `path:12` or `path:12:4` as the path and its line.
fn split_line(named: &str) -> (&str, Option<u32>) {
    let mut path = named;
    let mut line = None;
    for _ in 0..2 {
        match path.rsplit_once(':') {
            Some((before, number))
                if !before.is_empty()
                    && !number.is_empty()
                    && number.chars().all(|c| c.is_ascii_digit()) =>
            {
                line = number.parse().ok().or(line);
                path = before;
            }
            _ => break,
        }
    }
    (path, line)
}

/// A file's name as Claude writes one: no spaces, the characters of a path,
/// and either a folder in it or an extension with a letter in it.
fn looks_like_path(path: &str) -> bool {
    if path.len() < 3 || path.len() > PATH_MAX || path.contains("://") {
        return false;
    }
    let allowed = |c: char| c.is_alphanumeric() || "_-./\\~@+".contains(c);
    // `C:\dev\app` keeps its drive.
    let body = match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic() => &path[2..],
        _ => path,
    };
    if !body.chars().all(allowed) || body.ends_with('/') || body.ends_with('\\') {
        return false;
    }
    let name = body.rsplit(['/', '\\']).next().unwrap_or(body);
    let extension = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, extension)| extension);
    body.contains(['/', '\\'])
        || extension.is_some_and(|extension| {
            extension.len() <= 6 && extension.chars().any(|c| c.is_ascii_alphabetic())
        })
}

/// The link to `named` (a path, maybe with its line) when it is a file of
/// the workspace.
fn link_to(
    named: &str,
    ws: &str,
    checks: &mut usize,
    is_file: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let (path, _) = split_line(named);
    if !looks_like_path(path) || *checks >= CHECKS_MAX {
        return None;
    }
    *checks += 1;
    is_file(path).then(|| format!("{SCHEME}{ws}|{named}"))
}

/// `source` with the files it names linked (see the module), code blocks
/// untouched. `is_file` says whether a path, as written, is a file of the
/// workspace.
pub fn link_files(source: &str, ws: &str, is_file: &dyn Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(source.len() + 64);
    let mut fence: Option<(char, usize)> = None;
    let mut checks = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let run_of = |mark: char| trimmed.chars().take_while(|c| *c == mark).count();
        if let Some((mark, count)) = fence {
            let run = run_of(mark);
            if indent < 4 && run >= count && trimmed[run..].trim().is_empty() {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        let opens = ['`', '~']
            .into_iter()
            .map(|mark| (mark, run_of(mark)))
            .find(|(mark, run)| *run >= 3 && !(*mark == '`' && trimmed[*run..].contains('`')));
        if indent < 4
            && let Some(opened) = opens
        {
            fence = Some(opened);
            out.push_str(line);
            continue;
        }
        // An indented block is code too.
        if indent >= 4 || line.starts_with('\t') {
            out.push_str(line);
            continue;
        }
        out.push_str(&link_line(line, ws, &mut checks, is_file));
    }
    out
}

/// One line of prose: its inline code that names a file, and its links to
/// a relative path that is one, made links to the file.
fn link_line(line: &str, ws: &str, checks: &mut usize, is_file: &dyn Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        let tick = rest.find('`');
        let link = rest.find("](");
        match (tick, link) {
            // Inline code: as many backticks close it as open it.
            (Some(at), _) if link.is_none_or(|link| at < link) => {
                let run = rest[at..].chars().take_while(|c| *c == '`').count();
                let fence = &rest[at..at + run];
                out.push_str(&rest[..at]);
                let after = &rest[at + run..];
                let Some(close) = after.find(fence) else {
                    out.push_str(&rest[at..]);
                    break;
                };
                let code = &after[..close];
                let inside_link = out.ends_with('[');
                match (!inside_link)
                    .then(|| link_to(code.trim(), ws, checks, is_file))
                    .flatten()
                {
                    Some(url) => out.push_str(&format!("[{fence}{code}{fence}]({url})")),
                    None => out.push_str(&format!("{fence}{code}{fence}")),
                }
                rest = &after[close + run..];
            }
            // A link: its destination, when it is a file of the workspace.
            (_, Some(at)) => {
                out.push_str(&rest[..at + 2]);
                let after = &rest[at + 2..];
                let Some(close) = after.find(')') else {
                    out.push_str(after);
                    break;
                };
                let destination = &after[..close];
                let bare = destination.strip_prefix("file://").unwrap_or(destination);
                // `path#L12`, as editors link a line, read as `path:12`.
                let named = match bare.split_once("#L") {
                    Some((path, line))
                        if line.chars().all(|c| c.is_ascii_digit()) && !line.is_empty() =>
                    {
                        format!("{path}:{line}")
                    }
                    Some((path, _)) => path.to_string(),
                    None => bare.to_string(),
                };
                let web = destination.contains("://") || destination.starts_with("mailto:");
                match (!web)
                    .then(|| link_to(&named, ws, checks, is_file))
                    .flatten()
                {
                    Some(url) => out.push_str(&url),
                    None => out.push_str(destination),
                }
                out.push(')');
                rest = &after[close + 1..];
            }
            (None, None) => {
                out.push_str(rest);
                break;
            }
            (Some(_), None) => unreachable!("matched above"),
        }
    }
    out
}

/// Whether `path`, as Claude wrote it, is a file under `folder` (or, when
/// absolute, a file at all).
pub fn is_file_in(folder: &str, path: &str) -> bool {
    let named = Path::new(path);
    if named.is_absolute() {
        return named.is_file();
    }
    !folder.is_empty() && Path::new(folder).join(named).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(path: &str) -> bool {
        [
            "src/search/index.ts",
            "Cargo.toml",
            "README.md",
            "/abs/notes.txt",
        ]
        .contains(&path)
    }

    fn linked(source: &str) -> String {
        link_files(source, "w1", &files)
    }

    #[test]
    fn inline_code_naming_a_file_of_the_workspace_becomes_a_link_to_it() {
        assert_eq!(
            linked("Edited `src/search/index.ts:42` and `Cargo.toml`.\n"),
            "Edited [`src/search/index.ts:42`](hangar-file:w1|src/search/index.ts:42) and \
             [`Cargo.toml`](hangar-file:w1|Cargo.toml).\n"
        );
        // Not a file there, or not a path: as written.
        for kept in [
            "Run `pnpm test` then `cargo build`.\n",
            "See `src/gone.ts` and `config.port`.\n",
            "Version `v1.2.3`, `a b.ts`.\n",
        ] {
            assert_eq!(linked(kept), kept);
        }
    }

    #[test]
    fn a_link_to_a_relative_file_is_made_one_to_the_workspace_s_file() {
        assert_eq!(
            linked(
                "In [index.ts](src/search/index.ts#L12) and [docs](https://example.com/a.md).\n"
            ),
            "In [index.ts](hangar-file:w1|src/search/index.ts:12) and [docs](https://example.com/a.md).\n"
        );
        // Inline code that already is a link's text stays that link's.
        assert_eq!(
            linked("[`README.md`](README.md)\n"),
            "[`README.md`](hangar-file:w1|README.md)\n"
        );
    }

    #[test]
    fn code_blocks_are_never_touched() {
        let source = "```rust\nlet a = `src/search/index.ts`;\n```\n    `README.md`\n";
        assert_eq!(linked(source), source);
    }

    #[test]
    fn a_link_reads_back_its_workspace_its_path_and_its_line() {
        assert_eq!(
            file_of("hangar-file:w1|src/search/index.ts:42:7"),
            Some(FileLink {
                ws: "w1".into(),
                path: "src/search/index.ts".into(),
                line: Some(42),
            })
        );
        assert_eq!(
            file_of("hangar-file:w1|C:\\dev\\app\\main.rs"),
            Some(FileLink {
                ws: "w1".into(),
                path: "C:\\dev\\app\\main.rs".into(),
                line: None,
            })
        );
        assert_eq!(file_of("https://example.com"), None);
        assert_eq!(file_of("hangar-file:|x.rs"), None);
    }

    #[test]
    fn names_that_are_paths_and_names_that_are_not() {
        for path in [
            "src/a.rs",
            "Cargo.toml",
            "a/b",
            "C:\\dev\\x.rs",
            "~/x.md",
            "/abs/notes.txt",
        ] {
            assert!(looks_like_path(path), "{path}");
        }
        for word in [
            "v1.2.3",
            "a b",
            "http://x/y.md",
            "x",
            "src/",
            "foo()",
            "1.5",
        ] {
            assert!(!looks_like_path(word), "{word}");
        }
    }
}
