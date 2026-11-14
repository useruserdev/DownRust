// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Avatar} from '@astryxdesign/core/Avatar';
import {Badge} from '@astryxdesign/core/Badge';
import {Banner} from '@astryxdesign/core/Banner';
import {Icon} from '@astryxdesign/core/Icon';
import {Bug, ExternalLink, GitBranch, RefreshCw, Scale, Tags} from 'lucide-react';
import appIconUrl from '../../../assets/icon/icon.svg';
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
const LINKS: {labelKey: MessageKey; url: string; icon: typeof GitBranch}[] = [
  {labelKey: 'update.github', url: REPOSITORY, icon: GitBranch},
  {labelKey: 'update.releases', url: `${REPOSITORY}/releases`, icon: Tags},
  {labelKey: 'update.report', url: `${REPOSITORY}/issues/new/choose`, icon: Bug},
  {labelKey: 'update.licence', url: `${REPOSITORY}/blob/main/LICENSE`, icon: Scale},
];

/** Release notes longer than this many lines scroll inside a fixed height. */
const LONG_NOTES = 10;

/** Release notes as headings and bullet lines. */
function ReleaseNotes({notes}: {notes: ReturnType<typeof cleanReleaseNotes>}) {
  return (
    <VStack gap={1}>
      {notes.map((line, index) =>
        line.heading ? (
          <Text key={index} type="label">
            {line.text}
          </Text>
        ) : (
          <Text key={index}>{line.bullet ? `• ${line.text}` : line.text}</Text>
        ),
      )}
    </VStack>
  );
}

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

  const statusText =
    check.status === 'checking'
      ? t('update.checking')
      : check.status === 'error'
        ? t('update.checkFailed')
        : result && !result.available
          ? t('update.upToDate')
          : t('update.notChecked');

  return (
    <VStack gap={6}>
      <HStack gap={4} vAlign="center">
        <Avatar src={appIconUrl} alt="" shape="square" size="lg" />
        <StackItem size="fill">
          <VStack gap={1}>
            <HStack gap={2} vAlign="center">
              <Heading level={2}>DownRust</Heading>
              {info?.portable && <Badge variant="info" label={t('settings.about.portable')} />}
            </HStack>
            <Text color="secondary">
              {t('update.versionLine', {version: info?.version ?? '…'})} · {t('update.tagline')}
            </Text>
          </VStack>
        </StackItem>
      </HStack>

      <VStack gap={3}>
        <Heading level={3}>{t('update.title')}</Heading>
        {result?.available && result.version ? (
          <Banner
            status="info"
            title={t('update.newVersion', {version: result.version})}
            description={[
              t('update.current', {version: result.currentVersion}),
              result.date ? t('update.released', {date: result.date}) : null,
            ]
              .filter(Boolean)
              .join(' ')}
            collapsible={false}
            endContent={
              result.source === 'updater' ? (
                <Button label={t('update.install')} variant="primary" isLoading={installing} clickAction={install} />
              ) : (
                <Button
                  label={t('update.openPage')}
                  variant="primary"
                  icon={<Icon icon={ExternalLink} />}
                  clickAction={() => openLink(`${REPOSITORY}/releases/tag/v${result.version}`)}
                />
              )
            }>
            <VStack gap={3}>
              {installing && (
                <ProgressBar
                  label={t('update.installing')}
                  value={progress?.downloaded ?? 0}
                  max={progress?.total ?? undefined}
                  isIndeterminate={!progress?.total}
                />
              )}
              {notes.length > 0 && (
                <VStack gap={2}>
                  <Text type="label">{t('update.notesTitle')}</Text>
                  {notes.length > LONG_NOTES ? (
                    <ScrollableArea label={t('update.notesTitle')} height={240}>
                      <ReleaseNotes notes={notes} />
                    </ScrollableArea>
                  ) : (
                    <ReleaseNotes notes={notes} />
                  )}
                </VStack>
              )}
            </VStack>
          </Banner>
        ) : (
          <HStack gap={3} vAlign="center">
            <StackItem size="fill">
              <Text color="secondary">{statusText}</Text>
            </StackItem>
            <Button
              label={t('update.check')}
              variant="secondary"
              icon={<Icon icon={RefreshCw} />}
              isLoading={check.status === 'checking'}
              clickAction={runCheck}
            />
          </HStack>
        )}
        <Switch
          label={t('update.auto')}
          description={t('update.autoHint')}
          value={checkUpdates}
          onChange={value => updateSettings({checkUpdates: value})}
          labelPosition="start"
          labelSpacing="spread"
          width="100%"
        />
      </VStack>

      <VStack gap={3}>
        <Heading level={3}>{t('update.linksTitle')}</Heading>
        <HStack gap={2} wrap="wrap">
          {LINKS.map(link => (
            <Button
              key={link.url}
              label={t(link.labelKey)}
              variant="secondary"
              icon={<Icon icon={link.icon} />}
              clickAction={() => openLink(link.url)}
            />
          ))}
        </HStack>
      </VStack>

      <VStack gap={3}>
        <Heading level={3}>{t('update.foldersTitle')}</Heading>
        <MetadataList>
          <MetadataListItem label={t('settings.about.dataFolder')}>{info?.dataDir ?? '…'}</MetadataListItem>
          <MetadataListItem label={t('settings.about.logFolder')}>{info?.logDir ?? '…'}</MetadataListItem>
          <MetadataListItem label={t('settings.about.licence')}>GPL-3.0-or-later</MetadataListItem>
        </MetadataList>
        <Text type="supporting" color="secondary">
          {t('settings.about.licenceText')}
        </Text>
      </VStack>
    </VStack>
  );
}
