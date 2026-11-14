// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {HStack, VStack} from '@astryxdesign/core/Layout';
import {Badge} from '@astryxdesign/core/Badge';
import {Heading, Text} from '@astryxdesign/core/Text';
import {MetadataList, MetadataListItem} from '@astryxdesign/core/MetadataList';
import {api} from '../../ipc/commands';
import type {AppInfo} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import {t} from '../../i18n';

/** About: version, data and log folders, and licence. */
export function AboutSection() {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    let active = true;
    api
      .getAppInfo()
      .then(value => {
        if (active) setInfo(value);
      })
      .catch(err => notify('error', errorText(err)));
    return () => {
      active = false;
    };
  }, []);

  return (
    <VStack gap={5}>
      <HStack gap={2} vAlign="center">
        <Heading level={2}>{t('settings.section.about')}</Heading>
        {info?.portable && <Badge variant="info" label={t('settings.about.portable')} />}
      </HStack>
      <MetadataList>
        <MetadataListItem label={t('settings.about.version')}>{info?.version ?? '…'}</MetadataListItem>
        <MetadataListItem label={t('settings.about.dataFolder')}>{info?.dataDir ?? '…'}</MetadataListItem>
        <MetadataListItem label={t('settings.about.logFolder')}>{info?.logDir ?? '…'}</MetadataListItem>
        <MetadataListItem label={t('settings.about.licence')}>GPL-3.0-or-later</MetadataListItem>
      </MetadataList>
      <Text type="supporting" color="secondary">{t('settings.about.licenceText')}</Text>
    </VStack>
  );
}
