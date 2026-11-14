// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {HStack, VStack} from '@astryxdesign/core/Layout';
import {Badge} from '@astryxdesign/core/Badge';
import {Button} from '@astryxdesign/core/Button';
import {Heading, Text} from '@astryxdesign/core/Text';
import {MetadataList, MetadataListItem} from '@astryxdesign/core/MetadataList';
import {ProgressBar} from '@astryxdesign/core/ProgressBar';
import {ScrollableArea} from '@astryxdesign/core/ScrollableArea';
import {Switch} from '@astryxdesign/core/Switch';
import {cleanReleaseNotes} from '../../app/releaseNotes';
import {api} from '../../ipc/commands';
import {onEvent} from '../../ipc/events';
import type {AppInfo, UpdateInfo, UpdateProgress} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import {useSettings} from '../../state/settings';
import {t} from '../../i18n';
import type {MessageKey} from '../../i18n';

const REPOSITORY = 'https://github.com/useruserdev/DownRust';

/** Links shown at the foot of the About page. */
const LINKS: {labelKey: MessageKey; url: string}[] = [
  {labelKey: 'update.github', url: REPOSITORY},
  {labelKey: 'update.releases', url: `${REPOSITORY}/releases`},
  {labelKey: 'update.report', url: `${REPOSITORY}/issues/new/choose`},
  {labelKey: 'update.licence', url: `${REPOSITORY}/blob/main/LICENSE`},
];

type CheckState =
  | {status: 'idle'}
  | {status: 'checking'}
  | {status: 'done'; info: UpdateInfo}
  | {status: 'error'};

/** About: app icon area, version, update check with release notes, links and data folders. */
export function AboutSection() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [check, setCheck] = useState<CheckState>({status: 'idle'});
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const checkUpdates = useSettings(state => state.value?.checkUpdates ?? true);
  const updateSettings = useSettings(state => state.update);

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

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    onEvent('update-progress', value => {
      if (active) setProgress(value);
    }).then(stop => {
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const runCheck = () => {
    setCheck({status: 'checking'});
    api
      .checkUpdate()
      .then(result => setCheck({status: 'done', info: result}))
      .catch(() => setCheck({status: 'error'}));
  };

  const install = () => {
    setInstalling(true);
    setProgress(null);
    api.installUpdate().catch(err => {
      setInstalling(false);
      notify('error', errorText(err));
    });
  };

  const openLink = (url: string) => {
    api.openLink(url).catch(err => notify('error', errorText(err)));
  };

  const result = check.status === 'done' ? check.info : null;
  const notes = result?.notes ? cleanReleaseNotes(result.notes) : [];

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

      <VStack gap={3}>
        <HStack gap={2} vAlign="center">
          <Button
            label={t('update.check')}
            variant="secondary"
            isLoading={check.status === 'checking'}
            clickAction={runCheck}
          />
          {check.status === 'checking' && <Text type="supporting" color="secondary">{t('update.checking')}</Text>}
        </HStack>
        {check.status === 'error' && <Text type="supporting" color="secondary">{t('update.checkFailed')}</Text>}
        {result && !result.available && <Text>{t('update.upToDate')}</Text>}
        {result?.available && result.version && (
          <VStack gap={3}>
            <Heading level={3}>{t('update.newVersion', {version: result.version})}</Heading>
            <Text type="supporting" color="secondary">{t('update.current', {version: result.currentVersion})}</Text>
            {result.date && (
              <Text type="supporting" color="secondary">{t('update.released', {date: result.date})}</Text>
            )}
            {notes.length > 0 && (
              <VStack gap={2}>
                <Text type="label">{t('update.notesTitle')}</Text>
                <ScrollableArea label={t('update.notesTitle')} height="100%">
                  <VStack gap={1}>
                    {notes.map((line, index) =>
                      line.heading ? (
                        <Text key={index} type="label">{line.text}</Text>
                      ) : (
                        <Text key={index}>{line.text}</Text>
                      ),
                    )}
                  </VStack>
                </ScrollableArea>
              </VStack>
            )}
            {result.source === 'updater' ? (
              <Button
                label={t('update.install')}
                variant="primary"
                isLoading={installing}
                clickAction={install}
              />
            ) : (
              <Button
                label={t('update.openPage')}
                variant="primary"
                clickAction={() => openLink(`${REPOSITORY}/releases/tag/v${result.version}`)}
              />
            )}
            {installing && (
              <ProgressBar
                label={t('update.installing')}
                value={progress?.downloaded ?? 0}
                max={progress?.total ?? undefined}
                isIndeterminate={!progress?.total}
              />
            )}
          </VStack>
        )}
      </VStack>

      <Switch
        label={t('update.auto')}
        description={t('update.autoHint')}
        value={checkUpdates}
        onChange={value => updateSettings({checkUpdates: value})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <VStack gap={2}>
        <Text type="label">{t('update.linksTitle')}</Text>
        {LINKS.map(link => (
          <Button
            key={link.url}
            label={t(link.labelKey)}
            variant="secondary"
            clickAction={() => openLink(link.url)}
          />
        ))}
      </VStack>
    </VStack>
  );
}
