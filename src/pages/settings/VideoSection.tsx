// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {open as openFile} from '@tauri-apps/plugin-dialog';
import {Badge} from '@astryxdesign/core/Badge';
import {Button} from '@astryxdesign/core/Button';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {Selector} from '@astryxdesign/core/Selector';
import {SegmentedControl, SegmentedControlItem} from '@astryxdesign/core/SegmentedControl';
import {Switch} from '@astryxdesign/core/Switch';
import {TextInput} from '@astryxdesign/core/TextInput';
import {inTauri} from '../../ipc/runtime';
import {api} from '../../ipc/commands';
import type {AppSettings, FfmpegInfo, MediaQuality, SiteFormat, SiteToolStatus} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

/** Video settings: the ffmpeg program, MP4 conversion, the preferred quality and yt-dlp for video sites. */
export function VideoSection({settings, update}: SectionProps) {
  const media = settings.engine.media;
  const site = settings.engine.site;
  const [ffmpeg, setFfmpeg] = useState<FfmpegInfo | null | undefined>(undefined);
  const [detecting, setDetecting] = useState(false);
  const [tool, setTool] = useState<SiteToolStatus | null | undefined>(undefined);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    api
      .siteToolStatus()
      .then(status => setTool(status))
      .catch(err => notify('error', errorText(err)));
  }, []);

  const detect = () => {
    setDetecting(true);
    api
      .detectFfmpeg()
      .then(info => setFfmpeg(info))
      .catch(err => notify('error', errorText(err)))
      .finally(() => setDetecting(false));
  };

  const fetchTool = () => {
    setWorking(true);
    api
      .siteToolUpdate()
      .then(status => setTool(status))
      .catch(err => notify('error', errorText(err)))
      .finally(() => setWorking(false));
  };

  async function browse() {
    if (!inTauri) return;
    try {
      const picked = await openFile({
        multiple: false,
        directory: false,
        filters: [{name: 'yt-dlp', extensions: ['exe']}],
      });
      if (typeof picked === 'string') update({engine: {site: {ytdlpPath: picked}}});
    } catch (err) {
      notify('error', errorText(err));
    }
  }

  const formatOptions: {value: SiteFormat; label: string}[] = [
    {value: 'best', label: t('site.best')},
    {value: '2160', label: '2160p'},
    {value: '1440', label: '1440p'},
    {value: '1080', label: '1080p'},
    {value: '720', label: '720p'},
    {value: '480', label: '480p'},
    {value: 'audio', label: t('site.audioOnly')},
  ];

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('video.section')}</Heading>

      <HStack gap={2} vAlign="end">
        <StackItem size="fill">
          <TextInput
            label={t('video.ffmpegPath')}
            placeholder={t('video.ffmpegPlaceholder')}
            value={media.ffmpegPath}
            onChange={value => update({engine: {media: {ffmpegPath: value}}})}
          />
        </StackItem>
        <Button label={t('video.detect')} variant="secondary" isLoading={detecting} clickAction={detect} />
      </HStack>
      {ffmpeg !== undefined && (
        <HStack gap={2} vAlign="center">
          {ffmpeg === null ? (
            <Text type="supporting" color="secondary">{t('video.notFound')}</Text>
          ) : (
            <>
              <Badge variant="success" label={t('video.found', {version: ffmpeg.version, path: ffmpeg.path})} />
            </>
          )}
        </HStack>
      )}

      <Switch
        label={t('video.convert')}
        value={media.convertToMp4}
        onChange={value => update({engine: {media: {convertToMp4: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <HStack gap={4} vAlign="center">
        <StackItem size="fill">
          <Text type="label">{t('video.quality')}</Text>
        </StackItem>
        <SegmentedControl
          label={t('video.quality')}
          value={media.quality}
          onChange={value => update({engine: {media: {quality: value as MediaQuality}}})}>
          <SegmentedControlItem value="best" label={t('video.best')} />
          <SegmentedControlItem value="worst" label={t('video.worst')} />
        </SegmentedControl>
      </HStack>

      <Heading level={3}>{t('site.settingsTitle')}</Heading>

      <HStack gap={2} vAlign="end">
        <StackItem size="fill">
          <TextInput
            label={t('site.ytdlpPath')}
            placeholder={t('site.ytdlpPlaceholder')}
            value={site.ytdlpPath}
            onChange={value => update({engine: {site: {ytdlpPath: value}}})}
          />
        </StackItem>
        <Button label={t('common.browse')} variant="secondary" isDisabled={!inTauri} clickAction={() => void browse()} />
      </HStack>
      {tool !== undefined && (
        <HStack gap={2} vAlign="center">
          {tool === null ? (
            <Text type="supporting" color="secondary">{t('site.notFound')}</Text>
          ) : (
            <Badge variant="success" label={t('site.found', {version: tool.version})} />
          )}
        </HStack>
      )}
      <HStack gap={2}>
        <Button
          label={tool ? t('site.update') : t('site.download')}
          variant="secondary"
          isLoading={working}
          clickAction={fetchTool}
        />
      </HStack>

      <Switch
        label={t('site.autoDownload')}
        value={site.autoDownload}
        onChange={value => update({engine: {site: {autoDownload: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      <Switch
        label={t('site.autoUpdate')}
        value={site.autoUpdate}
        onChange={value => update({engine: {site: {autoUpdate: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Selector
        label={t('site.defaultFormat')}
        options={formatOptions}
        value={site.defaultFormat}
        onChange={value => update({engine: {site: {defaultFormat: value as SiteFormat}}})}
      />
    </VStack>
  );
}
