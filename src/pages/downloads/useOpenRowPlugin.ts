// SPDX-License-Identifier: GPL-3.0-or-later
// Portions adapted from Astryx templates, Copyright (c) Meta Platforms, Inc. and affiliates (MIT); see THIRD_PARTY_NOTICES.md
import {useMemo} from 'react';
import type {TablePlugin} from '@astryxdesign/core/Table';
import type {DownloadRow} from './DownloadsTable';

const INTERACTIVE = 'button, input, a, label, [role="checkbox"], [role="button"], [role="menuitem"]';

export interface OpenRowOptions {
  openId: number | null;
  onOpen: (id: number) => void;
  onActivate: (id: number) => void;
}

/** Makes a row clickable to open it in the details panel, and marks the open row. */
export function useOpenRowPlugin(options: OpenRowOptions): TablePlugin<DownloadRow> {
  const {openId, onOpen, onActivate} = options;
  return useMemo<TablePlugin<DownloadRow>>(
    () => ({
      rowModel: 'flat',
      transformBodyRow: (props, item) => ({
        ...props,
        htmlProps: {
          ...props.htmlProps,
          tabIndex: 0,
          'aria-current': item.id === openId ? true : undefined,
          onClick: event => {
            if (!(event.target as HTMLElement).closest(INTERACTIVE)) onOpen(item.id);
          },
          onContextMenu: () => onOpen(item.id),
          onDoubleClick: () => onActivate(item.id),
          onKeyDown: event => {
            if (event.key === 'Enter') onActivate(item.id);
          },
        },
      }),
      transformBodyCell: (props, _column, item) =>
        item.id === openId
          ? {
              ...props,
              htmlProps: {
                ...props.htmlProps,
                style: {...props.htmlProps.style, backgroundColor: 'var(--color-accent-muted)'},
              },
            }
          : props,
    }),
    [openId, onOpen, onActivate],
  );
}
