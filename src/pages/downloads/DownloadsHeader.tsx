// SPDX-License-Identifier: GPL-3.0-or-later
import {Button} from '@astryxdesign/core/Button';
import {DropdownMenu} from '@astryxdesign/core/DropdownMenu';
import {HStack, LayoutHeader, StackItem} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {TextInput} from '@astryxdesign/core/TextInput';
import {Icon} from '@astryxdesign/core/Icon';
import {Search} from 'lucide-react';
import {open as pickFile} from '@tauri-apps/plugin-dialog';
import {errorText, notify} from '../../lib/notify';
import {api} from '../../ipc/commands';
import {inTauri} from '../../ipc/runtime';
import {useDownloads} from '../../state/downloads';
import {useStatusCounts} from '../../state/selectors';
import {useUi} from '../../state/ui';
import {t} from '../../i18n';
import {downloadActions} from './RowContextMenu';

/** Picks a list file, reads its addresses and opens the batch dialog with them. */
async function importList(): Promise<void> {
  if (!inTauri) return;
  try {
    const picked = await pickFile({
      multiple: false,
      filters: [
        {name: t('importList.filterIdm'), extensions: ['ief']},
        {name: t('importList.filterText'), extensions: ['txt']},
        {name: t('importList.filterWeb'), extensions: ['html', 'htm']},
        {name: t('importList.filterAll'), extensions: ['*']},
      ],
    });
    if (typeof picked !== 'string') return;
    const urls = await api.importDownloadList(picked);
    if (urls.length === 0) {
      notify('info', t('importList.none'));
      return;
    }
    useUi.getState().openBatch(urls.join('\n'));
  } catch (err) {
    notify('error', errorText(err));
  }
}

/** Page header: title, live counts, search and the actions for the selected download. */
export function DownloadsHeader() {
  const counts = useStatusCounts();
  const search = useDownloads(state => state.search);
  const setSearch = useDownloads(state => state.setSearch);
  const selected = useDownloads(state => (state.selectedId === null ? undefined : state.records[state.selectedId]));

  const status = selected?.status;
  const canPause = status === 'downloading' || status === 'queued';
  const canResume = status === 'paused' || status === 'error';
  const canRestart = status !== undefined && status !== 'queued';
  const id = selected?.id ?? null;

  return (
    <LayoutHeader hasDivider>
      <HStack gap={3} vAlign="center">
        <StackItem size="fill">
          <HStack gap={2} vAlign="center">
            <Heading level={1}>{t('nav.downloads')}</Heading>
            <Text type="supporting" color="secondary">
              {t('downloads.summary', {active: counts.active, queued: counts.queued})}
            </Text>
          </HStack>
        </StackItem>
        <TextInput
          label={t('downloads.search')}
          isLabelHidden
          placeholder={t('downloads.search')}
          startIcon={<Icon icon={Search} size="sm" />}
          hasClear
          size="sm"
          value={search}
          onChange={setSearch}
        />
        <Button
          label={t('common.resume')}
          variant="secondary"
          size="sm"
          isDisabled={id === null || !canResume}
          clickAction={() => (id === null ? undefined : downloadActions.resume(id))}
        />
        <Button
          label={t('common.pause')}
          variant="secondary"
          size="sm"
          isDisabled={id === null || !canPause}
          clickAction={() => (id === null ? undefined : downloadActions.pause(id))}
        />
        <Button
          label={t('common.restart')}
          variant="secondary"
          size="sm"
          isDisabled={id === null || !canRestart}
          clickAction={() => (id === null ? undefined : downloadActions.restart(id))}
        />
        <Button
          label={t('common.remove')}
          variant="secondary"
          size="sm"
          isDisabled={id === null}
          clickAction={() => (id === null ? undefined : downloadActions.openRemove(id))}
        />
        <DropdownMenu
          button={{label: t('common.more'), variant: 'ghost', size: 'sm'}}
          items={[
            {label: t('menu.addSeveral'), onClick: () => useUi.getState().openBatch()},
            {label: `${t('importList.title')}…`, onClick: () => void importList()},
            {label: t('menu.pauseAll'), onClick: () => void downloadActions.pauseAll()},
            {label: t('menu.resumeAll'), onClick: () => void downloadActions.resumeAll()},
          ]}
        />
      </HStack>
    </LayoutHeader>
  );
}
