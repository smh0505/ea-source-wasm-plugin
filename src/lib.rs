//! WASM `SourcePlugin` for EA app / Origin games. All registry access goes through the `host`
//! interface, since guest code is sandboxed.
//!
//! `launch()` is dead code in practice, same as the Steam plugin: `executable_path` is a real
//! `origin2://game/launch/?offerIds=<contentID>` URI (a genuinely OS-registered protocol
//! handler, not a pseudo-URI like GOG's `gog://` or Xbox's `xbox://`), so `library.ts`'s generic
//! `openUrl()` branch handles it with no EA-specific code at all.

#[allow(warnings)]
mod bindings;

use bindings::exports::gamelib::plugin::source_plugin::{GameEntry, Guest};
use bindings::gamelib::plugin::host;

struct EaPlugin;

struct EaGame {
    content_id: String,
    display_name: String,
    install_dir: String,
}

/// Every subkey is a real EA-purchased title's contentID - EA-specific by construction, no
/// heuristic filter needed. Has no InstallLocation of its own though (see below).
const ORIGIN_GAMES_KEY: &str = "SOFTWARE\\WOW6432Node\\Origin Games";

/// The standard "Programs and Features" Uninstall registry has `InstallLocation`, keyed by a
/// random installer GUID rather than contentID - the only link is matching `DisplayName` text
/// exactly. Filtered to `Publisher: "Electronic Arts, Inc."` to avoid unrelated software.
const UNINSTALL_KEY: &str = "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

fn find_install_dirs_by_title() -> std::collections::HashMap<String, String> {
    let mut by_title = std::collections::HashMap::new();
    let Ok(entry_ids) = host::list_registry_keys("HKLM", UNINSTALL_KEY) else {
        return by_title; // best-effort - just means no install_dir, not a scan failure
    };

    for entry_id in entry_ids {
        let key = format!("{}\\{}", UNINSTALL_KEY, entry_id);
        let publisher = host::read_registry_string("HKLM", &key, "Publisher");
        if publisher.as_deref() != Some("Electronic Arts, Inc.") {
            continue;
        }
        let (Some(display_name), Some(install_location)) = (
            host::read_registry_string("HKLM", &key, "DisplayName"),
            host::read_registry_string("HKLM", &key, "InstallLocation"),
        ) else {
            continue;
        };
        by_title.insert(display_name, install_location);
    }

    by_title
}

fn find_ea_games() -> Result<Vec<EaGame>, String> {
    let content_ids = host::list_registry_keys("HKLM", ORIGIN_GAMES_KEY)?;
    let install_dirs_by_title = find_install_dirs_by_title();
    let mut games = Vec::new();

    for content_id in content_ids {
        let key = format!("{}\\{}", ORIGIN_GAMES_KEY, content_id);
        let Some(display_name) = host::read_registry_string("HKLM", &key, "DisplayName") else {
            continue;
        };
        // Origin Games only means "you own this," not "it's currently installed" - matching
        // every other source plugin's scan() semantics here (installed games only, same as
        // Steam/GOG/Epic/Xbox/Ubisoft), a title with no install folder that actually still
        // exists on disk is skipped rather than shown as a phantom "installed" game (e.g. an
        // uninstalled-but-still-owned title, or a stale Uninstall-registry leftover).
        let Some(install_dir) = install_dirs_by_title.get(&display_name) else {
            continue;
        };
        if host::request_read_scope(install_dir).is_err() {
            continue;
        }

        games.push(EaGame {
            content_id,
            display_name,
            install_dir: install_dir.clone(),
        });
    }

    Ok(games)
}

fn to_game_entry(game: &EaGame) -> GameEntry {
    GameEntry {
        id: format!("ea-{}", game.content_id),
        title: game.display_name.clone(),
        executable_path: format!("origin2://game/launch/?offerIds={}", game.content_id),
        platform: "ea".to_string(),
        cover_art_url: None,
        // Feeds the host's folder-based playtime tracking - always present now, since
        // find_ea_games only includes games with a real, currently-existing install folder.
        install_dir: Some(game.install_dir.clone()),
    }
}

impl Guest for EaPlugin {
    fn scan() -> Result<Vec<GameEntry>, String> {
        Ok(find_ea_games()?.iter().map(to_game_entry).collect())
    }

    fn launch(_entry: GameEntry) -> Result<(), String> {
        // Never actually reachable - see the module doc comment.
        Err("launch() is not used for EA entries - the host launches origin2:// URIs \
             directly via its own OS protocol handler."
            .to_string())
    }

    fn get_install_status(entry: GameEntry) -> Result<bool, String> {
        Ok(find_ea_games()?
            .iter()
            .any(|game| format!("ea-{}", game.content_id) == entry.id))
    }
}

bindings::export!(EaPlugin with_types_in bindings);
