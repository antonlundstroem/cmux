use std::path::Path;
use std::time::SystemTime;

pub fn shorten_home(path: &Path) -> String {
    let s = path.display().to_string();
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if let Some(rest) = s.strip_prefix(home.as_ref()) {
            return format!("~{rest}");
        }
    }
    s
}

pub fn fmt_duration(secs: u64, suffix: &str) -> String {
    if secs < 60 {
        format!("{secs}s{suffix}")
    } else if secs < 3600 {
        format!("{}m{suffix}", secs / 60)
    } else if secs < 86400 {
        format!("{}h{suffix}", secs / 3600)
    } else {
        format!("{}d{suffix}", secs / 86400)
    }
}

pub fn fmt_age(now: SystemTime, then: SystemTime) -> String {
    match now.duration_since(then) {
        Ok(dur) => fmt_duration(dur.as_secs(), " ago"),
        Err(_) => String::new(),
    }
}
