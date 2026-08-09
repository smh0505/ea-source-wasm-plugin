//! WASM `SourcePlugin` for EA (EA app / Origin), ported from research recorded in the
//! game-library-client's `.claude/devlog.md` (Milestone 16). All OS access (registry) goes
//! through the `host` interface instead of direct Win32/registry crates, since guest code is
//! sandboxed.
//!
//! `launch()` is implemented for contract-completeness (the WIT `source-plugin` interface
//! requires it) but is architecturally dead code in practice, same as the Steam plugin: the
//! host app's `library.ts` never calls a plugin's `launch()` for already-imported games - it
//! dispatches purely off the stored `executable_path` string. Here that string is a real
//! `origin2://game/launch/?offerIds=<contentID>` URI (verified as a genuinely OS-registered
//! protocol handler - `HKCR\origin2\shell\open\command` invokes `EALauncher.exe "%1"` - not a
//! pseudo-URI needing plugin-routed dispatch the way GOG's `gog://` or Xbox's `xbox://` do), so
//! `library.ts`'s existing generic `openUrl()` branch handles it with no EA-specific code at
//! all.

#[allow(warnings)]
mod bindings;

use bindings::exports::gamelib::plugin::source_plugin::{GameEntry, Guest};
use bindings::gamelib::plugin::host;

struct EaPlugin;

struct EaGame {
    content_id: String,
    display_name: String,
    install_dir: Option<String>,
}

/// Every subkey under this registry key is a real EA-purchased title's contentID - unlike
/// Xbox's AppX package repository (which lists every installed package on the system, games
/// and system components alike), this key is EA-specific by construction, so no heuristic
/// "is this a game" filter is needed at all. It has no InstallLocation value of its own though
/// (see `find_install_dirs_by_title` below for where that actually comes from).
const ORIGIN_GAMES_KEY: &str = "SOFTWARE\\WOW6432Node\\Origin Games";

/// The `Origin Games` key gives contentID + title but never an install path. The standard
/// Windows "Programs and Features" Uninstall registry does have one (`InstallLocation`), but
/// it's keyed by a random installer GUID, not contentID - the only link between the two is
/// matching `DisplayName` text exactly (verified: both said "Unravel™" identically for the same
/// real install). Filtered to `Publisher: "Electronic Arts, Inc."` so this doesn't pick up
/// unrelated installed software that happens to share a display name.
const UNINSTALL_KEY: &str = "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

fn find_install_dirs_by_title() -> std::collections::HashMap<String, String> {
    let mut by_title = std::collections::HashMap::new();
    let Ok(entry_ids) = host::list_registry_keys("HKLM", UNINSTALL_KEY) else {
        return by_title; // best-effort - a lookup failure here just means no install_dir, not a scan failure
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
        if let Some(display_name) = host::read_registry_string("HKLM", &key, "DisplayName") {
            let install_dir = install_dirs_by_title.get(&display_name).cloned();
            games.push(EaGame {
                content_id,
                display_name,
                install_dir,
            });
        }
    }

    Ok(games)
}

fn to_game_entry(game: &EaGame) -> GameEntry {
    GameEntry {
        id: format!("ea-{}", game.content_id),
        title: game.display_name.clone(),
        // A real OS-registered URI (see module doc comment) - the host launches this directly
        // via its own protocol handler, same as steam://, not via this plugin's launch().
        executable_path: format!(
            "origin2://game/launch/?offerIds={}",
            game.content_id
        ),
        platform: "ea".to_string(),
        cover_art_url: None,
        // Feeds the host's folder-based playtime tracking (launcher.rs::track_folder_playtime)
        // for this URI-launched entry - without it, the host has no folder to poll and playtime
        // silently never gets recorded. `None` only when the Uninstall-registry title match
        // above didn't find one; scan()/launch() still work either way, just without playtime.
        install_dir: game.install_dir.clone(),
    }
}

impl Guest for EaPlugin {
    fn scan() -> Result<Vec<GameEntry>, String> {
        Ok(find_ea_games()?.iter().map(to_game_entry).collect())
    }

    fn launch(_entry: GameEntry) -> Result<(), String> {
        // Never actually reachable (see the module doc comment) - documented as a real error
        // rather than calling host::spawn-process on an "origin2://..." URI, which would just
        // fail anyway (a URI can't be spawned as a process).
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
