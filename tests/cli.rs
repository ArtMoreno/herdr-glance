#[allow(dead_code)]
#[path = "../src/herdr.rs"]
mod herdr;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
use serde_json::Value;
use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn protocol_cache_and_prompt_latency() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("cli-check-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let fake = dir.join(if cfg!(windows) {
        "fake-herdr.exe"
    } else {
        "fake-herdr"
    });
    assert!(Command::new("rustc")
        .args(["--edition=2021", "tests/support/fake-herdr.rs", "-o"])
        .arg(&fake)
        .status()
        .unwrap()
        .success());
    let h = herdr::Herdr {
        session: "fixture".into(),
        workspace: "w-demo".into(),
        binary: fake.to_str().unwrap().into(),
    };
    let rows = h.list().unwrap();
    assert_eq!(rows.len(), 5);
    let mut session = h.clone();
    session.workspace = "*".into();
    assert_eq!(session.list().unwrap().len(), 6);
    assert_ne!(session.cache_path(), h.cache_path());
    let blocked = rows[0].clone();
    assert_eq!(
        h.read(&blocked, "recent-unwrapped").unwrap(),
        "first\nsecond"
    );
    assert_eq!(h.read(&blocked, "detection").unwrap(), "Which icon set?");
    h.focus(&blocked).unwrap();
    fs::write(dir.join("replaced"), "").unwrap();
    assert!(h.focus(&blocked).is_err());
    let log = fs::read_to_string(dir.join("calls.log")).unwrap();
    assert_eq!(log.matches("|focus|").count(), 1);
    assert!(!dir.join("unexpected-mutation").exists());
    fs::remove_file(dir.join("replaced")).unwrap();
    fs::write(dir.join("duplicate"), "").unwrap();
    h.focus(&blocked).unwrap();
    assert!(fs::read_to_string(dir.join("calls.log"))
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .ends_with("|focus|w-demo:p2"));
    let cli = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_glance"));
        c.env("HERDR_ENV", "1")
            .env("HERDR_WORKSPACE_ID", "w-demo")
            .env("HERDR_SESSION", "fixture")
            .env("HERDR_GLANCE_BIN", &fake)
            .env("HERDR_BIN_PATH", "missing-herdr-override-must-win")
            .env("HERDR_GLANCE_CACHE_DIR", dir.join("cache"))
            .env("XDG_CONFIG_HOME", &dir)
            .env("APPDATA", &dir);
        c
    };
    let denied = cli().env_remove("HERDR_ENV").arg("line").output().unwrap();
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("HERDR_ENV=1"));
    #[cfg(windows)]
    {
        // Herdr supplies a verbatim Windows root; PowerShell Join-Path rejects it.
        let root = dir.join("plugin path & spaces");
        fs::create_dir_all(root.join("target/release")).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_glance"),
            root.join("target/release/glance.exe"),
        )
        .unwrap();
        let manifest: toml::Value = toml::from_str(include_str!("../herdr-plugin.toml")).unwrap();
        let argv = manifest["panes"][1]["command"].as_array().unwrap();
        let out = Command::new(argv[0].as_str().unwrap())
            .args(argv[1..].iter().map(|v| v.as_str().unwrap()))
            .env("HERDR_PLUGIN_ROOT", root.canonicalize().unwrap())
            .env_remove("HERDR_ENV")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains("HERDR_ENV=1"));
    }
    assert!(!cli()
        .env_remove("HERDR_WORKSPACE_ID")
        .arg("line")
        .status()
        .unwrap()
        .success());
    assert!(cli().arg("--refresh").status().unwrap().success());
    // Plugin launches supply Herdr's executable even when it is not on PATH.
    assert!(cli()
        .env_remove("HERDR_GLANCE_BIN")
        .env("HERDR_BIN_PATH", &fake)
        .arg("--refresh")
        .status()
        .unwrap()
        .success());
    let out = cli().args(["line", "--plain"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "🧠1 💤1 🙋1 ✅1 ❓1"
    );
    let out = cli()
        .args(["line", "--plain", "--no-emoji"])
        .output()
        .unwrap();
    assert!(!out.stdout.contains(&27));
    assert!(String::from_utf8_lossy(&out.stdout).contains("working:1"));
    let out = cli().arg("line").output().unwrap();
    assert!(out.stdout.contains(&27));
    assert!(cli()
        .args(["--refresh", "--all-workspaces"])
        .status()
        .unwrap()
        .success());
    let out = cli()
        .args(["line", "--plain", "--no-emoji", "--all-workspaces"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("working:2"));
    let cache = fs::read_dir(dir.join("cache"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.extension().is_some_and(|e| e == "json")
                && serde_json::from_slice::<Value>(&fs::read(p).unwrap()).unwrap()["workspace"]
                    == "w-demo"
        })
        .unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    assert!(value.get("text").is_none());
    // A successful refresh replaces an existing cache (including on Windows).
    assert!(cli().arg("--refresh").status().unwrap().success());
    // Stale-but-usable data stays visible while the detached process refreshes it.
    value["at"] = (herdr::now() - 3).into();
    fs::write(&cache, serde_json::to_vec(&value).unwrap()).unwrap();
    let out = cli()
        .args(["line", "--plain", "--no-emoji"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("working:1"));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let fresh = fs::read(&cache)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        if fresh.is_some_and(|v| v["at"].as_u64().unwrap() > value["at"].as_u64().unwrap())
            && !cache.with_extension("lock").exists()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "detached refresh never completed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // Keep the asynchronous scope-rejection probe separate from later expiry probes.
    let scope_dir = dir.join("scope-cache");
    fs::create_dir_all(&scope_dir).unwrap();
    let scope_cache = scope_dir.join(cache.file_name().unwrap());
    // A payload copied into the wrong scope's path must not supply its counts.
    value["workspace"] = "wrong-workspace".into();
    value["at"] = herdr::now().into();
    fs::write(&scope_cache, serde_json::to_vec(&value).unwrap()).unwrap();
    fs::write(scope_cache.with_extension("lock"), "held").unwrap();
    let out = cli()
        .env("HERDR_GLANCE_CACHE_DIR", &scope_dir)
        .args(["line", "--plain", "--no-emoji"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "unknown");
    assert!(cli()
        .env("HERDR_GLANCE_CACHE_DIR", &scope_dir)
        .arg("--refresh")
        .status()
        .unwrap()
        .success());
    let future_lock = fs::File::create(cache.with_extension("lock")).unwrap();
    future_lock
        .set_modified(std::time::SystemTime::now() + Duration::from_secs(3600))
        .unwrap();
    drop(future_lock);
    assert!(cli().arg("--refresh").status().unwrap().success());
    assert!(!cache.with_extension("lock").exists());
    value["workspace"] = "w-demo".into();
    value["at"] = 0.into();
    fs::write(&cache, serde_json::to_vec(&value).unwrap()).unwrap();
    fs::write(dir.join("slow"), "").unwrap();
    let start = Instant::now();
    let out = cli().args(["line", "--plain"]).output().unwrap();
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "cold line waited for slow Herdr"
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "❓");
    let start = Instant::now();
    assert!(h.list().is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
    fs::remove_file(dir.join("slow")).unwrap();
    fs::write(dir.join("broken"), "").unwrap();
    assert!(h.list().is_err());
}
