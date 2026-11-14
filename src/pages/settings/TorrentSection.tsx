// SPDX-License-Identifier: GPL-3.0-or-later
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Switch} from '@astryxdesign/core/Switch';
import {VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import type {AppSettings} from '../../ipc/types';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

const KIBIBYTE = 1024;
const DEFAULT_UPLOAD_KB = 512;

/** Torrent settings: listening port, peer discovery, sharing after the download and upload speed. */
export function TorrentSection({settings, update}: SectionProps) {
  const torrent = settings.engine.torrent;
  const uploadKb = torrent.uploadLimit === null ? null : Math.round(torrent.uploadLimit / KIBIBYTE);

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('torrent.title')}</Heading>
      <Text type="supporting">{t('torrent.restartHint')}</Text>

      <NumberInput
        label={t('torrent.listenPort')}
        value={torrent.listenPort}
        onChange={value => update({engine: {torrent: {listenPort: value}}})}
        min={0}
        max={65535}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <Switch
        label={t('torrent.enableDht')}
        description={t('torrent.enableDhtHint')}
        value={torrent.enableDht}
        onChange={value => update({engine: {torrent: {enableDht: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Switch
        label={t('torrent.enableUpnp')}
        value={torrent.enableUpnp}
        onChange={value => update({engine: {torrent: {enableUpnp: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Switch
        label={t('torrent.seedAfterFinish')}
        description={t('torrent.seedAfterFinishHint')}
        value={torrent.seedAfterFinish}
        onChange={value => update({engine: {torrent: {seedAfterFinish: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Switch
        label={t('torrent.limitUpload')}
        value={torrent.uploadLimit !== null}
        onChange={value =>
          update({engine: {torrent: {uploadLimit: value ? DEFAULT_UPLOAD_KB * KIBIBYTE : null}}})
        }
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      {torrent.uploadLimit !== null && (
        <NumberInput
          label={t('torrent.uploadSpeed')}
          units={t('units.kbps')}
          value={uploadKb ?? DEFAULT_UPLOAD_KB}
          onChange={value => update({engine: {torrent: {uploadLimit: value * KIBIBYTE}}})}
          min={1}
          isIntegerOnly
          width={160}
        />
      )}
    </VStack>
  );
}
