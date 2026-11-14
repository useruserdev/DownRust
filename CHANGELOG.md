# Changelog

All notable changes to DownRust are listed here. The newest version is first.

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
