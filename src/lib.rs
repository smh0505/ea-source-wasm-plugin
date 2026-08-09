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
}

/// Every subkey under this registry key is a real EA-purchased title's contentID - unlike
/// Xbox's AppX package repository (which lists every installed package on the system, games
/// and system components alike), this key is EA-specific by construction, so no heuristic
/// "is this a game" filter is needed at all.
const ORIGIN_GAMES_KEY: &str = "SOFTWARE\\WOW6432Node\\Origin Games";

fn find_ea_games() -> Result<Vec<EaGame>, String> {
    let content_ids = host::list_registry_keys("HKLM", ORIGIN_GAMES_KEY)?;
    let mut games = Vec::new();

    for content_id in content_ids {
        let key = format!("{}\\{}", ORIGIN_GAMES_KEY, content_id);
        if let Some(display_name) = host::read_registry_string("HKLM", &key, "DisplayName") {
            games.push(EaGame {
                content_id,
                display_name,
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
        install_dir: None,
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
