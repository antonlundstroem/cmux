use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ResumableSession {
    pub id: String,              // session uuid (jsonl filename without extension)
    pub project_dir: PathBuf,    // decoded absolute path
    pub last_modified: SystemTime,
    /// First user message in the session, un-truncated but with newlines
    /// collapsed to spaces. Callers can truncate for list rendering.
    pub first_user_msg: String,
}

/// Scan ~/.claude/projects for resumable sessions.
///
/// For each project directory, pick the single newest `.jsonl` file and parse
/// it just far enough to extract the first user message as a preview. Sorted
/// newest-first by the jsonl file's mtime.
pub fn load() -> Vec<ResumableSession> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let projects_dir = PathBuf::from(home).join(".claude/projects");

    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&projects_dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let project_dir = decode_project_name(name);

        let Some((jsonl_path, mtime)) = newest_jsonl(&path) else {
            continue;
        };
        let Some(id) = jsonl_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };
        let first_user_msg = first_user_message(&jsonl_path).unwrap_or_default();

        out.push(ResumableSession {
            id,
            project_dir,
            last_modified: mtime,
            first_user_msg,
        });
    }

    out.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    out
}

/// `-home-anlu-work-infrastructure-git-main` → `/home/anlu/work/infrastructure/git/main`
///
/// Claude Code encodes the cwd by replacing `/` with `-`. Unfortunately this
/// isn't losslessly reversible (a dash in a dirname is indistinguishable from
/// a path separator), but for the common case of normal directory names it
/// works well enough for a preview.
fn decode_project_name(encoded: &str) -> PathBuf {
    let stripped = encoded.strip_prefix('-').unwrap_or(encoded);
    PathBuf::from(format!("/{}", stripped.replace('-', "/")))
}

fn newest_jsonl(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        match &best {
            Some((_, t)) if *t >= mtime => {}
            _ => best = Some((p, mtime)),
        }
    }
    best
}

/// Read just the JSONL lines we need to find the first user message whose
/// `content` is a plain string (skip tool_result-only turns). Newlines are
/// collapsed to spaces so callers can safely truncate.
fn first_user_message(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("user") {
            continue;
        }
        // message.content may be a string OR an array of content blocks.
        // We only want string content — the first one is the user's initial prompt.
        let content = v.get("message").and_then(|m| m.get("content"))?;
        if let Some(s) = content.as_str() {
            let cleaned: String = s
                .chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect();
            return Some(cleaned);
        }
    }
    None
}

/// Truncate to `n` characters with an ellipsis. Public so both the list row
/// renderer and any caller that wants a short label can share it.
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_normal_path() {
        assert_eq!(
            decode_project_name("-home-anlu-ctui"),
            PathBuf::from("/home/anlu/ctui")
        );
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn truncate_long() {
        let s = "a".repeat(100);
        let out = truncate(&s, 10);
        assert_eq!(out.chars().count(), 11); // 10 + ellipsis
    }

    #[test]
    fn truncate_preserves_unicode() {
        // "héllo world" is 11 chars; truncate to 6 keeps "héllo " then appends ellipsis.
        assert_eq!(truncate("héllo world", 6), "héllo …");
    }
}
