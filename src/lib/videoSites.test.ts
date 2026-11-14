// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import {isVideoSite} from './videoSites';

describe('isVideoSite', () => {
  it('matches video hosts and their subdomains', () => {
    expect(isVideoSite('https://www.youtube.com/watch?v=abc')).toBe(true);
    expect(isVideoSite('https://youtu.be/abc')).toBe(true);
    expect(isVideoSite('https://m.vimeo.com/12345')).toBe(true);
    expect(isVideoSite('https://rutube.ru/video/1/')).toBe(true);
  });

  it('needs a video path for some sites', () => {
    expect(isVideoSite('https://vk.com/video-1_2')).toBe(true);
    expect(isVideoSite('https://vk.com/friends')).toBe(false);
    expect(isVideoSite('https://x.com/user/status/1')).toBe(true);
    expect(isVideoSite('https://x.com/user')).toBe(false);
    expect(isVideoSite('https://www.instagram.com/reel/abc/')).toBe(true);
    expect(isVideoSite('https://www.instagram.com/someone/')).toBe(false);
    expect(isVideoSite('https://www.facebook.com/watch/?v=1')).toBe(true);
    expect(isVideoSite('https://www.facebook.com/someone')).toBe(false);
    expect(isVideoSite('https://ok.ru/video/12')).toBe(true);
    expect(isVideoSite('https://ok.ru/group/1')).toBe(false);
    expect(isVideoSite('https://www.reddit.com/r/x/comments/abc/t/')).toBe(true);
  });

  it('rejects other addresses', () => {
    expect(isVideoSite('https://notyoutube.com/watch')).toBe(false);
    expect(isVideoSite('https://example.com/video.mp4')).toBe(false);
    expect(isVideoSite('ftp://youtube.com/x')).toBe(false);
    expect(isVideoSite('youtube.com/watch?v=abc')).toBe(false);
    expect(isVideoSite('magnet:?xt=urn:btih:abc')).toBe(false);
  });
});
