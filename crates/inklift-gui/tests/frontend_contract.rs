//! The frontend and the Tauri config have to agree about how the API reaches
//! JavaScript. They are edited in different files and nothing connects them, so
//! a mismatch produces a window that renders perfectly and does nothing at all:
//! the very first line of the script throws, and no listener is ever attached.
//!
//! This is exactly what shipped once. These tests are the tripwire.

use std::fs;
use std::path::PathBuf;

fn gui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn config() -> serde_json::Value {
    let text = fs::read_to_string(gui_dir().join("tauri.conf.json")).expect("config must exist");
    serde_json::from_str(&text).expect("config must be valid JSON")
}

fn scripts() -> Vec<(String, String)> {
    fs::read_dir(gui_dir().join("ui"))
        .expect("ui/ must exist")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(&p).expect("script must be readable"),
            )
        })
        .collect()
}

/// `withGlobalTauri` defaults to FALSE. A bundler-less frontend that reaches
/// for `window.__TAURI__` therefore needs it turned on explicitly.
#[test]
fn a_frontend_using_the_global_api_has_it_enabled() {
    let uses_global = scripts()
        .iter()
        .any(|(_, body)| body.contains("window.__TAURI__"));
    if !uses_global {
        return; // an npm/bundler frontend has no such requirement
    }
    let enabled = config()["app"]["withGlobalTauri"].as_bool().unwrap_or(false);
    assert!(
        enabled,
        "the frontend reads window.__TAURI__, but withGlobalTauri is not true in \
         tauri.conf.json. It defaults to false, so the global is undefined, the \
         first line of the script throws, and no event listener is ever attached — \
         a window that renders and does nothing."
    );
}

/// Every command the frontend invokes must be registered on the Rust side, or
/// the button fails only when someone presses it.
#[test]
fn every_invoked_command_is_registered() {
    let main_rs = fs::read_to_string(gui_dir().join("src/main.rs")).expect("main.rs");
    let handler = main_rs
        .split("generate_handler![")
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("main.rs must register a command handler")
        .to_string();

    let mut missing = Vec::new();
    for (file, body) in scripts() {
        let mut rest = body.as_str();
        while let Some(i) = rest.find("invoke(\"") {
            rest = &rest[i + 8..];
            let name = rest.split('"').next().unwrap_or("");
            if !name.is_empty() && !handler.contains(name) {
                missing.push(format!("{file}: {name}"));
            }
        }
    }
    assert!(missing.is_empty(), "invoked but never registered: {missing:?}");
}

/// The CSP must not forbid the app's own scripts, or the page loads mute.
#[test]
fn the_csp_permits_the_apps_own_scripts() {
    let csp = config()["app"]["security"]["csp"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if csp.is_empty() {
        return;
    }
    assert!(
        csp.contains("script-src 'self'") || csp.contains("script-src 'unsafe-inline' 'self'"),
        "script-src must allow 'self' or the frontend cannot load its own JS: {csp}"
    );
}

/// Every window the backend opens must have a page to load.
#[test]
fn every_window_target_exists() {
    let main_rs = fs::read_to_string(gui_dir().join("src/main.rs")).expect("main.rs");
    for page in ["overlay.html"] {
        if main_rs.contains(page) {
            assert!(
                gui_dir().join("ui").join(page).exists(),
                "{page} is opened by the backend but missing from ui/"
            );
        }
    }
    assert!(gui_dir().join("ui/index.html").exists());
}

/// Commands the frontend reaches through `window.__TAURI__.<plugin>` are plugin
/// commands, and tauri's ACL gates every one of those regardless of origin
/// (webview/mod.rs: `plugin_command.is_some() || ... && invoke.acl.is_none()`
/// rejects). Our own `generate_handler!` commands are exempt; these are not.
/// Without a capability granting them, the file picker and the overlay's
/// report-back fail at the moment the user presses the button.
#[test]
fn plugin_apis_the_frontend_uses_are_granted_by_a_capability() {
    // which plugin namespaces does the frontend actually touch?
    let mut needed: Vec<&str> = Vec::new();
    let all: String = scripts().iter().map(|(_, b)| b.clone()).collect();
    if all.contains("__TAURI__.dialog") {
        needed.push("dialog:default");
    }
    if all.contains("__TAURI__.event") {
        needed.push("core:default"); // event listen/emit live in core
    }
    if needed.is_empty() {
        return;
    }

    let dir = gui_dir().join("capabilities");
    let files: Vec<_> = fs::read_dir(&dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        !files.is_empty(),
        "the frontend uses plugin APIs {needed:?} but capabilities/ is empty or absent, \
         so tauri's ACL rejects every one of them"
    );

    let granted: String = files
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect();
    for perm in &needed {
        assert!(
            granted.contains(perm),
            "no capability grants {perm}, which the frontend needs"
        );
    }

    // A capability that omits a window does not apply to it. The overlay is a
    // separate window and reports its result through the same IPC.
    let main_rs = fs::read_to_string(gui_dir().join("src/main.rs")).expect("main.rs");
    let mut labels = vec!["main".to_string()];
    let mut rest = main_rs.as_str();
    while let Some(i) = rest.find("WebviewWindowBuilder::new(") {
        rest = &rest[i..];
        if let Some(q) = rest.find('"') {
            let label = rest[q + 1..].split('"').next().unwrap_or("").to_string();
            if !label.is_empty() {
                labels.push(label);
            }
            rest = &rest[q + 1..];
        } else {
            break;
        }
    }
    for label in labels {
        assert!(
            granted.contains(&format!("\"{label}\"")),
            "window {label:?} is not listed in any capability, so plugin commands \
             from it are rejected"
        );
    }
}

/// A frontend that dies on its first line must say so. This bug shipped once as
/// a window that rendered perfectly and ignored every click, with nothing in
/// the log — the cost was an entire debugging round.
#[test]
fn the_frontend_reports_a_missing_api_instead_of_dying_silently() {
    for (file, body) in scripts() {
        if !body.contains("window.__TAURI__") {
            continue;
        }
        let guarded = body.contains("if (!window.__TAURI__")
            || body.contains("if (!globalThis.__TAURI__")
            || body.contains("__TAURI__ === undefined");
        assert!(
            guarded,
            "{file} dereferences window.__TAURI__ with no guard: if it is ever \
             absent the module aborts on line one, no listener is attached, and \
             the window looks fine while doing nothing"
        );
    }
}

/// `withGlobalTauri` injects tauri's own `bundle.global.js` and nothing else.
/// Plugin JS APIs are NOT part of it: a plugin ships an `api-iife.js` that does
/// `Object.defineProperty(window.__TAURI__, "<ns>", ...)`, but nothing in the
/// Rust toolchain ever injects that file — the dialog plugin's own init script
/// only overrides window.alert/confirm. So a bundler-less frontend reaching for
/// `window.__TAURI__.dialog` gets undefined, and the button fails when pressed.
/// Either vendor the plugin's iife into ui/, or do that work in Rust.
#[test]
fn the_frontend_only_uses_api_namespaces_that_actually_get_injected() {
    // exported by tauri-2.x/scripts/bundle.global.js; confirmed against the
    // running app, which logs its namespaces at startup
    const INJECTED: &[&str] = &[
        "app", "core", "dpi", "event", "image", "menu", "mocks", "path", "tray",
        "webview", "webviewWindow", "window",
    ];

    let vendored: Vec<String> = fs::read_dir(gui_dir().join("ui"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    let mut bad = Vec::new();
    for (file, body) in scripts() {
        let mut rest = body.as_str();
        while let Some(i) = rest.find("__TAURI__.") {
            rest = &rest[i + 10..];
            let ns: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if ns.is_empty() || INJECTED.contains(&ns.as_str()) {
                continue;
            }
            // acceptable only if that plugin's iife is shipped in ui/
            let has_iife = vendored.iter().any(|v| v.contains(&ns) && v.ends_with(".js"));
            if !has_iife {
                bad.push(format!("{file}: __TAURI__.{ns}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "these namespaces are never injected and will be undefined at runtime: {bad:?}"
    );
}

/// A `catch` whose entire body is `console.error` is a swallowed failure: the
/// webview's console is invisible in normal use, so the user sees the action do
/// nothing. Every failure the user can cause must reach the user or the log.
#[test]
fn no_ui_catch_block_only_writes_to_the_console() {
    let mut swallowed = Vec::new();
    for (file, body) in scripts() {
        let mut rest = body.as_str();
        while let Some(i) = rest.find("catch (") {
            rest = &rest[i..];
            let Some(open) = rest.find('{') else { break };
            // the body up to its closing brace, which for these handlers is the
            // first one at the same nesting level
            let mut depth = 0usize;
            let mut end = open;
            for (j, c) in rest[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = open + j;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let block = &rest[open + 1..end];
            let speaks = block.contains("toast")
                || block.contains("textContent")
                || block.contains("ui_ready")
                || block.contains("throw")
                || block.trim().is_empty() // `catch (_) {}` with a stated reason
                || block.contains("/*");
            if !speaks && block.contains("console.") {
                swallowed.push(format!("{file}: catch {{{}}}", block.trim()));
            }
            rest = &rest[end.max(open + 1)..];
        }
    }
    assert!(
        swallowed.is_empty(),
        "these failures reach only the webview console, which nobody sees: {swallowed:#?}"
    );
}
