// SPDX-License-Identifier: GPL-3.0-or-later
// Video sites that yt-dlp can download. The same list lives in
// crates/downrust-core/src/util/video_sites.rs and extension/popup.js.

interface SiteRule {
  host: string;
  /** Parts a page address must contain. An empty list means any path. */
  paths: string[];
}

const RULES: SiteRule[] = [
  {host: 'youtube.com', paths: []},
  {host: 'youtu.be', paths: []},
  {host: 'youtube-nocookie.com', paths: []},
  {host: 'vimeo.com', paths: []},
  {host: 'dailymotion.com', paths: []},
  {host: 'twitch.tv', paths: []},
  {host: 'tiktok.com', paths: []},
  {host: 'vk.com', paths: ['video', 'clip']},
  {host: 'vkvideo.ru', paths: []},
  {host: 'rutube.ru', paths: []},
  {host: 'ok.ru', paths: ['/video']},
  {host: 'dzen.ru', paths: ['/video']},
  {host: 'x.com', paths: ['/status/']},
  {host: 'twitter.com', paths: ['/status/']},
  {host: 'instagram.com', paths: ['/p/', '/reel']},
  {host: 'facebook.com', paths: ['/watch', '/videos/']},
  {host: 'fb.watch', paths: []},
  {host: 'reddit.com', paths: ['/comments/']},
  {host: 'bilibili.com', paths: []},
  {host: 'soundcloud.com', paths: []},
  {host: 'bandcamp.com', paths: []},
  {host: 'coub.com', paths: []},
  {host: 'streamable.com', paths: []},
];

/** True when the text is an http or https page address of a listed video site. Subdomains match. */
export function isVideoSite(text: string): boolean {
  let url: URL;
  try {
    url = new URL(text.trim());
  } catch {
    return false;
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return false;
  const host = url.hostname.toLowerCase();
  const path = url.pathname.toLowerCase();
  return RULES.some(
    rule =>
      (host === rule.host || host.endsWith(`.${rule.host}`)) &&
      (rule.paths.length === 0 || rule.paths.some(part => path.includes(part))),
  );
}
