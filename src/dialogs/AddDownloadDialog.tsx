// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useMemo, useRef, useState} from 'react';
import {open as pickFolder} from '@tauri-apps/plugin-dialog';
import {Badge} from '@astryxdesign/core/Badge';
import {Banner} from '@astryxdesign/core/Banner';
import {Button} from '@astryxdesign/core/Button';
import {Card} from '@astryxdesign/core/Card';
import {Collapsible} from '@astryxdesign/core/Collapsible';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, Layout, LayoutContent, LayoutFooter, StackItem, VStack} from '@astryxdesign/core/Layout';
import {MetadataList, MetadataListItem} from '@astryxdesign/core/MetadataList';
import {Slider} from '@astryxdesign/core/Slider';
import {Spinner} from '@astryxdesign/core/Spinner';
import {Text} from '@astryxdesign/core/Text';
import {TextInput} from '@astryxdesign/core/TextInput';
import {IconButton} from '@astryxdesign/core/IconButton';
import {Icon} from '@astryxdesign/core/Icon';
import {ClipboardPaste, FolderOpen} from 'lucide-react';
import {readClipboardText, asHttpUrl} from '../app/useKeyboardShortcuts';
import {CategoryIcon, categoryForFileName, categoryLabel} from '../lib/categories';
import {findDuplicate} from '../lib/duplicates';
import {humanBytes} from '../lib/format';
import {statusLabel} from '../lib/status';
import {errorText, notify} from '../lib/notify';
import {linkTokenCount} from '../lib/urlPattern';
import {inTauri} from '../ipc/runtime';
import {api} from '../ipc/commands';
import type {AddRequest, DownloadKind} from '../ipc/types';
import {useDownloads} from '../state/downloads';
import {useSettings} from '../state/settings';
import {useUi} from '../state/ui';
import {t} from '../i18n';
import {isProbeableUrl, useProbe} from './useProbe';

const TARGET_DELAY_MS = 300;
const SHA256_PATTERN = /^[0-9a-fA-F]{64}$/;
const FALLBACK_CONNECTIONS = 8;

/** Playlist addresses: a .m3u8 is an HLS stream, a .mpd is a DASH manifest. */
function kindFromUrl(url: string): DownloadKind | null {
  const path = url.trim().split(/[?#]/)[0].toLowerCase();
  if (path.endsWith('.m3u8')) return 'hls';
  if (path.endsWith('.mpd')) return 'dash';
  return null;
}

/** Add download dialog, driven by `useUi().addDialog`. */
export function AddDownloadDialog() {
  const isOpen = useUi(state => state.addDialog.open);
  const seq = useUi(state => state.addDialog.seq);
  const closeAdd = useUi(state => state.closeAdd);

  if (!isOpen) return null;

  return (
    <Dialog
      isOpen
      onOpenChange={open => {
        if (!open) closeAdd();
      }}
      purpose="form"
      width={560}>
      <AddForm key={seq} />
    </Dialog>
  );
}

function AddForm() {
  const prefill = useUi(state => state.addDialog);
  const closeAdd = useUi(state => state.closeAdd);
  const openBatch = useUi(state => state.openBatch);
  const setView = useUi(state => state.setView);
  const upsert = useDownloads(state => state.upsert);
  const select = useDownloads(state => state.select);
  const records = useDownloads(state => state.records);
  const defaults = useSettings(state => state.value?.engine);

  const [url, setUrl] = useState(prefill.url);
  const [fileName, setFileName] = useState(prefill.fileName ?? '');
  const [saveDir, setSaveDir] = useState(defaults?.defaultDir ?? '');
  const [connections, setConnections] = useState(defaults?.defaultConnections ?? FALLBACK_CONNECTIONS);
  const [sha256, setSha256] = useState('');
  const userPickedDir = useRef(false);
  const probe = useProbe(url, prefill.headers);

  // An empty open takes the address from the clipboard when it holds a link. The address is
  // selected, so typing replaces it instead of adding to it.
  useEffect(() => {
    if (prefill.url !== '') return;
    void readClipboardText().then(text => {
      const link = asHttpUrl(text);
      if (link === null) return;
      setUrl(current => (current === '' ? link : current));
      window.requestAnimationFrame(() => {
        const active = document.activeElement;
        if (active instanceof HTMLInputElement && active.value === link) active.select();
      });
    });
  }, [prefill.url]);

  // A successful probe supplies the file name.
  useEffect(() => {
    if (probe.status === 'ok') setFileName(probe.info.fileName);
  }, [probe]);

  // The save folder follows the file name until the user picks a folder.
  useEffect(() => {
    const name = fileName.trim();
    if (name === '') return;
    const timer = window.setTimeout(() => {
      api
        .suggestTarget(name)
        .then(target => {
          if (!userPickedDir.current) setSaveDir(target.saveDir);
        })
        .catch(err => notify('error', errorText(err)));
    }, TARGET_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [fileName]);

  const kind: DownloadKind | null = prefill.kind ?? kindFromUrl(url);
  const sha256Trimmed = sha256.trim();
  const sha256Valid = sha256Trimmed === '' || SHA256_PATTERN.test(sha256Trimmed);
  const canSubmit = isProbeableUrl(url) && sha256Valid;
  const info = probe.status === 'ok' ? probe.info : null;
  const category = info?.category ?? categoryForFileName(fileName || url);
  const totalSize = info?.totalSize;
  const duplicate = useMemo(
    () =>
      findDuplicate(Object.values(records), {
        url: url.trim(),
        fileName: fileName.trim(),
        totalSize,
      }),
    [records, url, fileName, totalSize],
  );

  async function paste() {
    const link = asHttpUrl(await readClipboardText());
    if (link !== null) setUrl(link);
  }

  // A paste with two or more links (it arrives with its line breaks removed) opens the batch dialog.
  // Typing adds one character at a time and never switches dialogs.
  function changeAddress(value: string) {
    const pasted = value.length - url.length >= 8;
    const restored = value.replace(/(?!^)(?=https?:\/\/)/gi, '\n');
    if (pasted && linkTokenCount(restored) >= 2) {
      closeAdd();
      openBatch(restored);
      return;
    }
    setUrl(value);
  }

  function showExisting(id: number) {
    select(id);
    setView('downloads');
    closeAdd();
  }

  async function browse() {
    if (!inTauri) return;
    try {
      const picked = await pickFolder({directory: true, multiple: false, defaultPath: saveDir || undefined});
      if (typeof picked === 'string') {
        userPickedDir.current = true;
        setSaveDir(picked);
      }
    } catch (err) {
      notify('error', errorText(err));
    }
  }

  async function submit(start: boolean) {
    const request: AddRequest = {
      url: url.trim(),
      fileName: fileName.trim() || null,
      saveDir: saveDir || null,
      connections,
      speedLimit: null,
      start,
      referrer: prefill.referrer,
      headers: prefill.headers,
      probe: info,
      expectedSha256: sha256Trimmed === '' ? null : sha256Trimmed.toLowerCase(),
      kind,
    };
    try {
      const record = await api.addDownload(request);
      upsert(record);
      closeAdd();
    } catch (err) {
      notify('error', errorText(err));
    }
  }

  const probeStatus = probe.status === 'error' ? {type: 'error' as const, message: probe.message} : undefined;

  return (
    <Layout
      header={<DialogHeader title={t('add.title')} onOpenChange={closeAdd} />}
      content={
        <LayoutContent>
          <VStack gap={4}>
            <HStack gap={2} vAlign="end">
              <StackItem size="fill">
                <TextInput
                  label={t('common.address')}
                  value={url}
                  onChange={changeAddress}
                  placeholder="https://"
                  hasAutoFocus
                  status={probeStatus}
                  hasClear
                />
              </StackItem>
              <IconButton
                label={t('add.paste')}
                icon={<Icon icon={ClipboardPaste} />}
                tooltip={t('add.paste')}
                onClick={() => void paste()}
              />
            </HStack>

            {probe.status === 'checking' && <Spinner size="sm" label={t('add.checking')} />}

            {kind !== null && (
              <Badge
                variant="info"
                label={t('add.videoStream', {format: t(kind === 'hls' ? 'kind.hls' : 'kind.dash')})}
              />
            )}

            {info && (
              <Card variant="muted" padding={3}>
                <VStack gap={3}>
                  <HStack gap={2} vAlign="center">
                    <CategoryIcon category={category} size="md" />
                    <StackItem size="fill">
                      <TextInput label={t('add.fileName')} value={fileName} onChange={setFileName} />
                    </StackItem>
                  </HStack>
                  <MetadataList orientation="horizontal">
                    <MetadataListItem label={t('inspector.size')}>
                      {info.totalSize === null ? t('common.unknown') : humanBytes(info.totalSize)}
                    </MetadataListItem>
                    <MetadataListItem label={t('inspector.resume')}>
                      {info.resumable ? t('common.supported') : t('common.notSupported')}
                    </MetadataListItem>
                    <MetadataListItem label={t('add.type')}>{info.contentType ?? t('common.unknown')}</MetadataListItem>
                    <MetadataListItem label={t('add.category')}>{categoryLabel(category)}</MetadataListItem>
                  </MetadataList>
                </VStack>
              </Card>
            )}

            {!info && fileName.trim() !== '' && (
              <TextInput label={t('add.fileName')} value={fileName} onChange={setFileName} />
            )}

            <HStack gap={2} vAlign="end">
              <StackItem size="fill">
                <TextInput label={t('add.saveTo')} value={saveDir} isReadOnly />
              </StackItem>
              <Button
                label={t('common.browse')}
                variant="secondary"
                icon={<Icon icon={FolderOpen} />}
                onClick={() => void browse()}
                isDisabled={!inTauri}
              />
            </HStack>

            <Slider
              label={t('common.connections')}
              min={1}
              max={32}
              step={1}
              value={connections}
              onChange={setConnections}
              valueDisplay="text"
              formatValue={value => String(value)}
            />
            <Collapsible trigger={t('add.moreOptions')} defaultIsOpen={false}>
              <VStack gap={3}>
                <TextInput
                  label={t('add.sha256Label')}
                  description={t('add.sha256Hint')}
                  value={sha256}
                  onChange={setSha256}
                  status={sha256Valid ? undefined : {type: 'error', message: t('errors.sha256')}}
                  hasClear
                />
              </VStack>
            </Collapsible>
            {probe.status === 'error' && <Text type="supporting">{t('add.probeFailed')}</Text>}
            {duplicate?.kind === 'link' && (
              <Banner
                status="warning"
                title={t('add.duplicateLink', {name: duplicate.record.fileName, status: statusLabel(duplicate.record.status)})}
                endContent={
                  <Button
                    label={t('add.showIt')}
                    variant="secondary"
                    size="sm"
                    clickAction={() => showExisting(duplicate.record.id)}
                  />
                }
              />
            )}
            {duplicate?.kind === 'file' && <Banner status="warning" title={t('add.duplicateFile')} />}
          </VStack>
        </LayoutContent>
      }
      footer={
        <LayoutFooter>
          <HStack gap={2} hAlign="end">
            <Button
              label={t('common.addWithoutStart')}
              variant="secondary"
              isDisabled={!canSubmit}
              clickAction={() => submit(false)}
            />
            <Button
              label={t('common.startDownload')}
              variant="primary"
              isDisabled={!canSubmit}
              clickAction={() => submit(true)}
            />
          </HStack>
        </LayoutFooter>
      }
    />
  );
}
