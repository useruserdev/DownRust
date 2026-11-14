// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Badge} from '@astryxdesign/core/Badge';
import {Button} from '@astryxdesign/core/Button';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {SegmentedControl, SegmentedControlItem} from '@astryxdesign/core/SegmentedControl';
import {Switch} from '@astryxdesign/core/Switch';
import {TextInput} from '@astryxdesign/core/TextInput';
import {api} from '../../ipc/commands';
import type {AppSettings, FfmpegInfo, MediaQuality} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

/** Video settings: the ffmpeg program used for merging, MP4 conversion and the preferred quality. */
export function VideoSection({settings, update}: SectionProps) {
  const media = settings.engine.media;
  const [ffmpeg, setFfmpeg] = useState<FfmpegInfo | null | undefined>(undefined);
  const [detecting, setDetecting] = useState(false);

  const detect = () => {
    setDetecting(true);
    api
      .detectFfmpeg()
      .then(info => setFfmpeg(info))
      .catch(err => notify('error', errorText(err)))
      .finally(() => setDetecting(false));
  };

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
    </VStack>
  );
}
