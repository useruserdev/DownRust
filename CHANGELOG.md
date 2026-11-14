# Changelog

All notable changes to DownRust are listed here. The newest version is first.

## 0.2.0 (2026-11-14)

### Video sites

- Videos from YouTube, VK, Rutube, Twitch, TikTok, Vimeo and other sites, with a choice of quality or audio only. DownRust uses yt-dlp, downloads it when it is missing and updates it every week.
- "Find video on this page" for other pages, and a "Download video from this page" button in the browser extension.

### New protocols

- Torrents: magnet links, `.torrent` addresses and local `.torrent` files, with progress, speed and peers.
- FTP, FTPS and SFTP, with resume from the same byte and speed limits.

### Downloads

- Mirrors: one file from several servers at the same time. A mirror with a different size is ignored.
- Speed schedule: different speed limits for hours and days.
- Automatic SHA-256 check when the server publishes a `.sha256` file or a `SHA256SUMS` list.
- Import of a download list from an IDM export file (`.ief`), a text file or a web page.

### Browser

- The extension connects to DownRust by itself. You no longer copy the port and the token.
- "Download all links with DownRust" and "Download selected links" on a page, with filters by file type.

### App

- Updates inside the app: Settings → About shows the new version and its changes, and installs it with one click.
- Links to the GitHub repository, all releases and problem reports in Settings → About.
- Download progress on the Windows taskbar button.
- Jump list tasks: New download, Pause all and Resume all.
- Command line: `DownRust.exe add <address>`, `--new`, `--pause-all`, `--resume-all`, and `downrust://` links.
- Optional handling of `magnet:` links.

## 0.1.0 — first release (2026-11-14)

First public version of DownRust for Windows.

### Downloads

- Segmented downloads over several connections, with dynamic splitting of the largest remaining part.
- Resume after pause, restart or crash, and automatic retries after errors.
- Speed limits for all downloads and for each download.
- Queue with a fixed order, a schedule for running hours and days, and an action when the queue is empty.
- Categories with automatic sorting into `Downloads\DownRust`.
- SHA-256 verification, refresh of expired links and optional disk space reservation.

### Video

- HLS streams, including AES-128 encrypted streams, with a choice of quality.
- DASH streams and separate audio tracks, with ffmpeg.

### Browser

- Browser extension for Chrome, Edge, Firefox and Zen.
- Capture of browser downloads, a context menu item and detection of HLS and DASH streams.

### Network

- System, direct and manual proxy, including SOCKS5, with proxy rules for each site.
- HTTP Basic logins for each site, stored in Windows Credential Manager.

### Desktop

- Tray icon, floating drop zone, clipboard monitor and drag and drop.
- Batch add with number patterns, warnings about duplicate downloads and statistics.
- Optional scan of finished files with Windows Security.
- Start with Windows, single instance and portable mode.
- English and Russian interface, with light and dark themes.
