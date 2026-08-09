# EA Source Plugin (Concourse)

A [Concourse](https://github.com/smh0505/Concourse) `source` plugin that scans installed
EA app / Origin games and adds them to your library.

## Install

Paste this URL into Concourse's Settings -> Source tab -> Add Plugin:

```
https://github.com/smh0505/ea-source-wasm-plugin/releases/latest/download/plugin.json
```

No special permissions needed - this plugin only reads a fixed registry key, and launches via
a real OS-registered URI protocol rather than spawning any process itself.

## How detection works

Every subkey under `HKLM\SOFTWARE\WOW6432Node\Origin Games` is a real EA-purchased title's
contentID, with a `DisplayName` value giving its title - this registry key is EA-specific by
construction (unlike, say, Xbox's AppX package repository, which lists every installed system
package and needs a heuristic filter), so no filtering is needed here at all.

## Launching

`origin2://game/launch/?offerIds=<contentID>` is a genuinely OS-registered protocol handler
(`HKCR\origin2\shell\open\command` invokes EA Desktop's own `EALauncher.exe`), verified against
a real installed game (Unravel) before this plugin was written - not a pseudo-URI needing this
plugin's own `launch()` to be called, the same way `steam://` already works in Concourse.

## Building locally

```sh
rustup target add wasm32-wasip1
cargo install cargo-component --locked
cargo component build --release
```

Produces `target/wasm32-wasip1/release/ea_source_wasm_plugin.wasm`.
