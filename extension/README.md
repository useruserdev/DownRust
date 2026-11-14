# DownRust browser extension

Sends browser downloads and "Download with DownRust" link requests to the DownRust download manager.

The extension works only when DownRust is running with the browser bridge enabled. If DownRust does not answer, the browser downloads the file as usual.

## Get the port and token

1. Open DownRust, then Settings, then the Browser section.
2. Turn the bridge on. Note the port (default 52151) and the token.
3. Open the extension options page and enter the same port and token. Click Save, then Test connection.

## Load unpacked

Chrome and Edge:

1. Open `chrome://extensions` (Chrome) or `edge://extensions` (Edge).
2. Turn on Developer mode.
3. Click Load unpacked and select this `extension` folder.

Firefox:

1. Open `about:debugging#/runtime/this-firefox`.
2. Click Load Temporary Add-on and select the `manifest.json` file in this folder.
3. On the options page, click Grant access to all sites.

Firefox removes temporary add-ons when it restarts. Load it again after a restart.

## Zen and other Firefox-based browsers

Zen, LibreWolf, Floorp and Waterfox use the Firefox engine and run the same extension.

Build the package from the repository root:

```
npm run extension:build
```

This writes `dist-extension/downrust-<version>.xpi`.

Install it permanently in Zen:

1. Open `about:config` and set `xpinstall.signatures.required` to `false`.
2. Open `about:addons`, click the gear button, choose Install Add-on From File and pick the `.xpi` file.
3. Open the DownRust options page, click Grant access to all sites, then enter the port and token.

If your build of Zen still asks for a signed add-on, sign the package for free as an unlisted add-on on addons.mozilla.org with `npx web-ext sign --channel unlisted --source-dir extension` and your own API keys, then install the signed `.xpi` the same way. For a quick test without installing, use Load Temporary Add-on from `about:debugging` as described for Firefox.

## Video

The extension watches the requests of each tab. When a page loads an HLS playlist (`.m3u8`) or a DASH manifest (`.mpd`), the toolbar badge shows how many streams were found. Open the DownRust menu on the page and press Download next to a stream to send it to the app.

- The list clears when the tab navigates to another address or closes.
- Start playing the video before you open the menu, so the player loads its playlist.
- HLS streams download without extra tools. A DASH stream and HLS streams with a separate audio track need ffmpeg. Set its path in the app under Settings → Video, or put `ffmpeg` next to DownRust.
- The extension needs the `webRequest` permission to see these requests. It only reads the address and the Content-Type of each request and does not change them.

## License

GPL-3.0-or-later
