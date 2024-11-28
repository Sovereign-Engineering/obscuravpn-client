# Obscura VPN Desktop (via Tauri)

Tauri desktop UI wrapped around daemon.

## Developer Setup

- [Tauri prerequisites](https://tauri.app/guides/getting-started/prerequisites),
- Install [NodeJS](https://nodejs.org/en/) via `nvm`
  1. Install [`nvm`](https://github.com/nvm-sh/nvm) or [`nvm-windows`](https://github.com/coreybutler/nvm-windows/releases)
  2. Open up an elevated command prompt (Windows) or a normal terminal on non-Windows OS
  3. `nvm install lts` (or `lts/*` on non-Windows) which will output a version (X.Y.Z) that was installed
  4. `nvm use lts` (`lts/*` on non-Windows)
- Install `pnpm` using `corepack enable`
- Run `pnpm install` to install frontend dependencies (`--frozen-lockfile` for reproducible builds)
- For testing, you need to `cargo install tauri-driver`, [Install Selenium IDE](https://www.selenium.dev/selenium-ide/), and on Windows add [msedgedriver.exe x64](https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/) to your Path environment variable.
  - A good place to store `msedgedriver.exe` is `C:\Windows` if you have administrative privileges
  - If tests are hanging on Windows, you will need to update `msedgedriver.exe` to the latest version
  - Use the Selenium IDE to open and save to the test/app.side file

To start developing, use `pnpm run dev`. If remote-sshing and you want to show the GUI on the

## Resources

### WSL Helpers

- `wslpath -w "$PWD"` print path for explorer
- Add the following to Windows Defender Exclusion list (Ubuntu is an example of a distro)
  - `\\wsl$\Ubuntu\`
  - `\\wsl.localhost\Ubuntu\`

### Purpose Related

TODO

### IDE

- [VS Code](https://code.visualstudio.com/) + [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer Extension](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### Libraries

- [Tauri (JS)](https://tauri.studio/docs/api/js/)
- [Tauri (Rust)](https://docs.rs/tauri/)
- [React Icons](https://react-icons.github.io/react-icons)
- [Mantine Docs](https://mantine.dev/pages/basics/)
- [Mantine Default Theme](https://github.com/mantinedev/mantine/blob/master/src/mantine-styles/src/theme/default-theme.ts)
- [react-18next Trans Component](https://react.i18next.com/latest/trans-component)

### App Icons

```sh
pnpm run tauri icon path/to/image.ext --output ./src-tauri/icons/
```

This will overwrite images in ./src-tauri/icons so ensure you are using `git`

### System Tray Creating Ico Files

This only applies when your source icon is not an ico.
I suggest keeping source images in the `git` repo.

```sh
pnpm run tauri icon ./src-tauri/icons/SystemTray1.png --output ./src-tauri/icons/SystemTray1
```

After running this, simply rename the ico file in the new directory and move it one level up. This file is referenced in `tauri.conf`. All `./src-tauri/icons/SystemTray*/` directories are ignored in [`.gitignore`](./.gitignore)

### Implementing Auto-Update

[Watch Tauri Part 10](https://youtu.be/ZXjlZBisYPQ)

These sample projects/code are licensed under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) so you
do not need to worry about violating my copyright. The backends can convert a github releases API response
into a Tauri-Updater friendly JSON response with a 5 minute cache and room for custom logic.

If you are using private repositories, you will need an API key from GitHub otherwise you will get 403ed.

- [Python Flask sample code](https://github.com/elibroftw/website/blob/master/blueprints/tauri_releases.py)
- [Rust Rocket sample code](https://github.com/elibroftw/rust-backend-tutorials/blob/master/src/tauri_releases.rs)

There are notes in the code for where to place custom version and platform checking logic. An example of custom logic is if you want to return the latest version only for certain app versions and another version for older app versions.

To test if auto-update works, in your `tauri.conf.json` file, add a localhost url like "http://[::1]:5001/tauri-releases/google-keep-desktop/{{target}}/{{current_version}}" to the start of the endpoints array. Your app version MUST be lower than the latest available version since the Tauri updater will perform a version on any valid json response.

### Calling Rust from the Frontend

```rs
#[tauri::command]
fn command1(window: tauri::Window) {
  println!("I was invoked from JS!");
}

#[tauri::command]
fn command2(arg: String) -> String {
  println!("I was invoked from JS!");
  return "hi frontend".into();
}

tauri::Builder::default()
  // ...
  .invoke_handler(tauri::generate_handler![command1, command2])
  // ...
```

```js

import { invoke } from '@tauri-apps/api/tauri'
// or const invoke = window.__TAURI__.invoke
// in an async function...
await invoke('command1');
await invoke('command2', {arg: 'two'}).then(msg => console.log(msg));
// ...
```

## Scripts in `package.json`

### `pnpm run dev`

An alias for `pnpm run tauri dev` which runs `pnpm run start` and opens a debug enabled window app that renders the frontend

### `pnpm run test`

Run integrated tests using selenium-webdriver on a release build of the application

### `pnpm run rls`

An alias for `pnpm run tauri build` which builds the frontend and bundles it into a Tauri release build

### `pnpm run update`

Upgrades packages/crates in `./packages.json` and `./src-tauri/Cargo.toml`. This will also cleanup rust builds

### `pnpm run build`

A performance optimized build of the front-end intended for use in production. Output is the `build` folder.

### `pnpm run start`

Run the app in the development mode ~~and opens [http://localhost:3000](http://localhost:3000) in your browser~~.
The page should reload when you make changes. You may also see lint errors in the console

### `pnpm run serve`

Serve `/build` with the global `serve` package

### `npm test`

Launches the test runner in the interactive watch mode.
See the section about [running tests](https://facebook.github.io/create-react-app/docs/running-tests) for more information.

## Tips and Trouble Shooting

- Slow computer / low storage / Frontend edits only? `pnpm run start`
  - no Tauri API access of course
- Publishing to the web? Edit `package.json`, `index.html`, `manifest.html`
- Don't want to Alt-Tab? Set `alwaysOnTop` to `true` in `tauri.conf.json`
- Broken npm sub-dependency? Use `resolutions: {subDependency: version}`
- Add `"devtools"` to Tauri features in `Cargo.toml` to get devtools in a production build
- If a cookie is not being set from cross-site, add `SameSite: 'lax'` when setting cookies
- Use `cd src-tauri && cargo clean` to fix abnormal bugs or issues
- Windows VSCode `pnpm install` package installation issues: **close** not reload all VS Code windows and retry
- Use `npx --yes npm-check-updates` to check which packages to upgrade, check their changelogs, and then perform updates
