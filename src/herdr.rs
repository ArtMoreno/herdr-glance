use crate::model::{self, Agent, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct Herdr {
    pub session: String,
    pub workspace: String,
    pub binary: String,
}
pub fn command(binary: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(binary);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.stdin(Stdio::null());
    c
}

pub fn detach_prompt_handles() -> Result<()> {
    // Windows otherwise inherits the prompt capture pipes even with child stdout=NUL.
    // The parent exits quickly, but shells wait for EOF until the refresher exits.
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE},
            System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE},
        };
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = GetStdHandle(id);
            if !handle.is_null()
                && handle != INVALID_HANDLE_VALUE
                && SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    }
    Ok(())
}
impl Herdr {
    pub fn from_env() -> Result<Self> {
        if env::var("HERDR_ENV").as_deref() != Ok("1") {
            return Err("Run glance inside Herdr (HERDR_ENV=1 is required).".into());
        }
        let workspace = env::var("HERDR_WORKSPACE_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or("HERDR_WORKSPACE_ID is required; launch from a Herdr pane.")?;
        let session = env::var("HERDR_SESSION").unwrap_or_else(|_| "default".into());
        Ok(Self {
            session,
            workspace,
            binary: env::var("HERDR_GLANCE_BIN")
                .unwrap_or_else(|_| if cfg!(windows) { "herdr.exe" } else { "herdr" }.into()),
        })
    }
    pub fn call(&self, args: &[&str]) -> Result<Value> {
        let v: Value = serde_json::from_str(&self.call_text(args)?)?;
        model::result(&v)?;
        Ok(v)
    }
    fn call_text(&self, args: &[&str]) -> Result<String> {
        let mut child = command(&self.binary)
            .args(["--session", &self.session])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let read = |pipe: Box<dyn Read + Send>| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.take(2_097_153).read_to_end(&mut bytes).map(|_| bytes)
            })
        };
        let out = read(Box::new(stdout));
        let err = read(Box::new(stderr));
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(3) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Herdr timed out after 3s".into());
            }
            thread::sleep(Duration::from_millis(5));
        };
        let bytes = out.join().map_err(|_| "output reader failed")??;
        let error = err.join().map_err(|_| "error reader failed")??;
        if !status.success() {
            return Err(format!(
                "Herdr command failed: {}",
                model::clean(&String::from_utf8_lossy(if error.is_empty() {
                    &bytes
                } else {
                    &error
                }))
            )
            .into());
        }
        if bytes.len() > 2_097_152 {
            return Err("Herdr response exceeds 2 MiB".into());
        }
        Ok(String::from_utf8(bytes)?)
    }
    pub fn list(&self) -> Result<Vec<Agent>> {
        model::agents(&self.call(&["agent", "list"])?, &self.workspace)
    }
    pub fn read(&self, a: &Agent, source: &str) -> Result<String> {
        let text = self.call_text(&[
            "pane", "read", &a.pane_id, "--source", source, "--lines", "40",
        ])?;
        let text = model::clean(&text);
        // Some Herdr builds return empty below viewport height; retry read-only and trim locally.
        if text.is_empty() && source == "recent-unwrapped" {
            let text = self.call_text(&[
                "pane", "read", &a.pane_id, "--source", source, "--lines", "200",
            ])?;
            let text = model::clean(&text);
            let lines: Vec<_> = text.lines().collect();
            return Ok(lines[lines.len().saturating_sub(40)..].join("\n"));
        }
        Ok(text)
    }
    pub fn focus(&self, selected: &Agent) -> Result<()> {
        let response = self.call(&["agent", "list"])?;
        let rows = model::agents(&response, &self.workspace)?;
        let all = model::result(&response)?["agents"]
            .as_array()
            .ok_or("missing agents array")?;
        let a = rows
            .iter()
            .find(|a| a.id() == selected.id())
            .ok_or("Selected agent exited or was replaced; focus cancelled")?;
        // Prefer the discovered unique name; duplicate/unnamed agents use their discovered pane ID.
        let target = a
            .name
            .as_deref()
            .filter(|n| {
                !n.is_empty()
                    && !n.starts_with('-')
                    && all.iter().filter(|x| x["name"].as_str() == Some(n)).count() == 1
            })
            .unwrap_or(&a.pane_id);
        self.call(&["agent", "focus", target])?;
        Ok(())
    }
    pub fn cache_path(&self) -> PathBuf {
        let base = env::var_os("HERDR_GLANCE_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::var_os("LOCALAPPDATA")
                    .or_else(|| env::var_os("XDG_CACHE_HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        env::var_os("HOME")
                            .map(|p| PathBuf::from(p).join(".cache"))
                            .unwrap_or_else(env::temp_dir)
                    })
                    .join("herdr-glance")
            });
        let scope = format!("{}\0{}\0{}", self.session, self.workspace, self.binary);
        let name: String = scope
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        // Keep filenames short on Windows; stable FNV-1a, scope is also verified in the payload.
        let hash = name.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        });
        base.join(format!("{hash:016x}.json"))
    }
    pub fn cache(&self) -> Option<Cache> {
        let c: Cache = serde_json::from_slice(&fs::read(self.cache_path()).ok()?).ok()?;
        (c.session == self.session && c.workspace == self.workspace && c.binary == self.binary)
            .then_some(c)
    }
    pub fn save(&self, rows: &[Agent]) -> Result<()> {
        let mut counts = [0; 5];
        for a in rows {
            counts[a.state()] += 1;
        }
        let c = Cache {
            at: now(),
            session: self.session.clone(),
            workspace: self.workspace.clone(),
            binary: self.binary.clone(),
            counts,
        };
        let path = self.cache_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, serde_json::to_vec(&c)?)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
    pub fn refresh(&self) -> Result<()> {
        let path = self.cache_path().with_extension("lock");
        fs::create_dir_all(path.parent().unwrap())?;
        if let Ok(meta) = fs::metadata(&path) {
            if meta
                .modified()
                .ok()
                .and_then(|m| m.elapsed().ok())
                .is_none_or(|age| age > Duration::from_secs(15))
            {
                let _ = fs::remove_file(&path);
            }
        }
        let Ok(_lock) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        else {
            return Ok(());
        };
        let r = self.list().and_then(|rows| self.save(&rows));
        drop(_lock);
        let _ = fs::remove_file(path);
        r
    }
}
#[derive(Serialize, Deserialize)]
pub struct Cache {
    pub at: u64,
    pub session: String,
    pub workspace: String,
    pub binary: String,
    pub counts: [usize; 5],
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
