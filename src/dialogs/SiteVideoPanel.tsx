// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useRef, useState} from 'react';
import {Banner} from '@astryxdesign/core/Banner';
import {Card} from '@astryxdesign/core/Card';
import {VStack} from '@astryxdesign/core/Layout';
import {MetadataList, MetadataListItem} from '@astryxdesign/core/MetadataList';
import {Selector} from '@astryxdesign/core/Selector';
import {Spinner} from '@astryxdesign/core/Spinner';
import {Text} from '@astryxdesign/core/Text';
import {api} from '../ipc/commands';
import type {HeaderPair, SiteFormat, SiteInfo} from '../ipc/types';
import {formatEta} from '../lib/format';
import {errorText} from '../lib/notify';
import {useSettings} from '../state/settings';
import {t} from '../i18n';

/** The file name and the options JSON that the Add dialog sends for a video page. */
export interface SiteChoice {
  fileName: string;
  options: string;
}

interface SiteVideoPanelProps {
  url: string;
  headers: HeaderPair[];
  /** Receives the choice, or null while the page is not read. */
  onChange: (choice: SiteChoice | null) => void;
}

const HEIGHT_FORMATS: {format: SiteFormat; height: number}[] = [
  {format: '2160', height: 2160},
  {format: '1440', height: 1440},
  {format: '1080', height: 1080},
  {format: '720', height: 720},
  {format: '480', height: 480},
];

/** Best, each standard height the page has, and audio only. */
function qualityOptions(info: SiteInfo): {value: SiteFormat; label: string}[] {
  const top = info.heights.length > 0 ? Math.max(...info.heights) : 0;
  const heights = HEIGHT_FORMATS.filter(({height}) => height <= top).map(({format, height}) => ({
    value: format,
    label: `${height}p`,
  }));
  return [{value: 'best', label: t('site.best')}, ...heights, {value: 'audio', label: t('site.audioOnly')}];
}

/** Reads a video page with yt-dlp and lets the user pick the quality. */
export function SiteVideoPanel({url, headers, onChange}: SiteVideoPanelProps) {
  const defaultFormat = useSettings(state => state.value?.engine.site.defaultFormat) ?? 'best';
  const [info, setInfo] = useState<SiteInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [format, setFormat] = useState<SiteFormat>(defaultFormat);
  const onChangeRef = useRef(onChange);
  const headersKey = JSON.stringify(headers);

  useEffect(() => {
    onChangeRef.current = onChange;
  });

  useEffect(() => {
    let active = true;
    setInfo(null);
    setError(null);
    api
      .siteInfo(url, JSON.parse(headersKey) as HeaderPair[])
      .then(result => {
        if (active) setInfo(result);
      })
      .catch(err => {
        if (active) setError(errorText(err));
      });
    return () => {
      active = false;
    };
  }, [url, headersKey]);

  const options = info ? qualityOptions(info) : [];
  const selected: SiteFormat = options.some(option => option.value === format) ? format : 'best';

  useEffect(() => {
    if (info === null) {
      onChangeRef.current(null);
      return;
    }
    const base = info.fileName.replace(/\.mp4$/i, '');
    onChangeRef.current({
      fileName: selected === 'audio' ? `${base}.m4a` : info.fileName,
      options: JSON.stringify({format: selected}),
    });
  }, [info, selected]);

  if (error !== null) {
    return <Banner status="error" title={t('site.readFailed', {error})} />;
  }
  if (info === null) {
    return <Spinner size="sm" label={t('site.loading')} />;
  }
  return (
    <VStack gap={3}>
      <Card variant="muted" padding={3}>
        <VStack gap={3}>
          <Text type="label">{info.title || info.fileName}</Text>
          <MetadataList orientation="horizontal">
            <MetadataListItem label={t('site.site')}>{info.site || t('common.unknown')}</MetadataListItem>
            <MetadataListItem label={t('site.duration')}>
              {info.durationSecs === null ? t('common.unknown') : formatEta(info.durationSecs)}
            </MetadataListItem>
          </MetadataList>
        </VStack>
      </Card>
      <Selector
        label={t('site.quality')}
        options={options}
        value={selected}
        onChange={value => setFormat(value as SiteFormat)}
      />
    </VStack>
  );
}
