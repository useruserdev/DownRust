// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useMemo, useState} from 'react';
import {StackItem, VStack} from '@astryxdesign/core/Layout';
import {Table, pixel, proportional, useTableSelection, useTableSortable} from '@astryxdesign/core/Table';
import type {TableColumn} from '@astryxdesign/core/Table';
import {EmptyState} from '@astryxdesign/core/EmptyState';
import {useMediaQuery} from '@astryxdesign/core/hooks';
import {Icon} from '@astryxdesign/core/Icon';
import {Download} from 'lucide-react';
import type {DownloadRecord, DownloadStatus} from '../../ipc/types';
import {useDownloads} from '../../state/downloads';
import {useVisibleRows} from '../../state/selectors';
import {useOpenRowPlugin} from './useOpenRowPlugin';
import {activateDownload, RowContextMenu} from './RowContextMenu';
import {BulkToolbar} from './BulkToolbar';
import {isEditableTarget} from '../../lib/keyboard';
import {t} from '../../i18n';
import {NameCell} from './cells/NameCell';
import {SizeCell} from './cells/SizeCell';
import {ProgressCell} from './cells/ProgressCell';
import {SpeedCell} from './cells/SpeedCell';
import {EtaCell} from './cells/EtaCell';
import {StatusCell} from './cells/StatusCell';
import {AddedCell} from './cells/AddedCell';

export interface DownloadRow extends Record<string, unknown> {
  id: number;
  record: DownloadRecord;
}

type SortKey = 'name' | 'size' | 'status' | 'added';
type SortDirection = 'ascending' | 'descending';
interface SortEntry {
  sortKey: SortKey;
  direction: SortDirection;
}

const STATUS_RANK: Record<DownloadStatus, number> = {
  downloading: 0,
  queued: 1,
  paused: 2,
  error: 3,
  completed: 4,
};

function compareRecords(key: SortKey, a: DownloadRecord, b: DownloadRecord): number {
  switch (key) {
    case 'name':
      return a.fileName.localeCompare(b.fileName);
    case 'size':
      return (a.totalSize ?? -1) - (b.totalSize ?? -1);
    case 'status':
      return STATUS_RANK[a.status] - STATUS_RANK[b.status];
    case 'added':
      return a.createdAt - b.createdAt;
  }
}

function sortRecords(records: DownloadRecord[], sort: SortEntry[]): DownloadRecord[] {
  if (sort.length === 0) return records;
  const [primary] = sort;
  const sign = primary.direction === 'ascending' ? 1 : -1;
  return [...records].sort((a, b) => sign * compareRecords(primary.sortKey, a, b));
}

/** Column definitions with their headers in the screen language. Built on each render so labels follow the language. */
function buildColumns(): TableColumn<DownloadRow>[] {
  return [
    {key: 'name', header: t('table.name'), width: proportional(3, {minWidth: 180}), sortable: true, renderCell: row => <NameCell record={row.record} />},
    {key: 'size', header: t('table.size'), width: pixel(80), align: 'end', sortable: true, renderCell: row => <SizeCell record={row.record} />},
    {key: 'progress', header: t('table.progress'), width: pixel(140), renderCell: row => <ProgressCell record={row.record} />},
    {key: 'speed', header: t('table.speed'), width: pixel(88), align: 'end', renderCell: row => <SpeedCell record={row.record} />},
    {key: 'eta', header: t('table.timeLeft'), width: pixel(76), align: 'end', renderCell: row => <EtaCell record={row.record} />},
    {key: 'status', header: t('table.status'), width: pixel(112), sortable: true, renderCell: row => <StatusCell record={row.record} />},
    {key: 'added', header: t('table.added'), width: pixel(120), sortable: true, renderCell: row => <AddedCell record={row.record} />},
  ];
}

function selectRow(id: number): void {
  useDownloads.getState().select(id);
}

function activateRow(id: number): void {
  const record = useDownloads.getState().records[id];
  if (record) activateDownload(record);
}

/** Live table of downloads. Progress cells subscribe to their own progress entries. */
export function DownloadsTable() {
  const visible = useVisibleRows();
  const selectedId = useDownloads(state => state.selectedId);
  const hasAnyRecord = useDownloads(state => Object.keys(state.records).length > 0);
  const detailsOpen = useDownloads(state => state.selectedId !== null && state.selectedId in state.records);
  const isNarrow = useMediaQuery('(max-width: 1024px)');
  const [sort, setSort] = useState<SortEntry[]>([]);

  // The Added column is hidden while the details panel is shown, so the other columns fit.
  const columns = useMemo(() => {
    const all = buildColumns();
    return isNarrow || !detailsOpen ? all : all.filter(column => column.key !== 'added');
  }, [isNarrow, detailsOpen]);

  // The Queued filter lists downloads in the order they will start, unless a column sort is chosen.
  const isQueueFilter = useDownloads(
    state => state.filter.kind === 'status' && state.filter.value === 'queued',
  );

  const rows = useMemo<DownloadRow[]>(() => {
    const ordered =
      isQueueFilter && sort.length === 0
        ? [...visible].sort((a, b) => a.queuePosition - b.queuePosition)
        : sortRecords(visible, sort);
    return ordered.map(record => ({id: record.id, record}));
  }, [visible, sort, isQueueFilter]);

  const checkedIds = useDownloads(state => state.checkedIds);
  const visibleIds = useMemo(() => rows.map(row => row.id), [rows]);
  const allChecked = visibleIds.length > 0 && visibleIds.every(id => checkedIds.has(id));
  const someChecked = visibleIds.some(id => checkedIds.has(id));

  // Ctrl+A selects every visible row while focus is inside the table.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
      if (event.key.toLowerCase() !== 'a') return;
      const target = event.target;
      if (isEditableTarget(target) || !(target instanceof HTMLElement) || target.closest('table') === null) return;
      event.preventDefault();
      useDownloads.getState().setChecked(visibleIds, true);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [visibleIds]);

  const selectionPlugin = useTableSelection<DownloadRow>({
    getIsItemSelected: row => checkedIds.has(row.id),
    onSelectItem: ({item, isSelected}) => useDownloads.getState().setChecked([item.id], isSelected),
    onSelectAll: ({isAllSelected}) => useDownloads.getState().setChecked(visibleIds, isAllSelected),
    getIsAllSelected: () => allChecked,
    getIsIndeterminate: () => someChecked && !allChecked,
    getRowLabel: row => row.record.fileName,
  });

  const sortPlugin = useTableSortable<DownloadRow, SortKey>({sort, onSortChange: setSort});
  const openPlugin = useOpenRowPlugin({openId: selectedId, onOpen: selectRow, onActivate: activateRow});

  if (rows.length === 0) {
    return (
      <EmptyState
        title={hasAnyRecord ? t('table.noMatch') : t('table.empty')}
        description={hasAnyRecord ? t('table.noMatchHint') : t('table.emptyHint')}
        icon={<Icon icon={Download} size="lg" />}
      />
    );
  }

  return (
    <VStack gap={0}>
      {checkedIds.size > 0 && <BulkToolbar ids={[...checkedIds]} />}
      <RowContextMenu>
        <StackItem size="fill">
          <Table
            data={rows}
            columns={columns}
            idKey="id"
            density="compact"
            dividers="rows"
            hasHover
            textOverflow="truncate"
            plugins={{selection: selectionPlugin, sort: sortPlugin, open: openPlugin}}
          />
        </StackItem>
      </RowContextMenu>
    </VStack>
  );
}
