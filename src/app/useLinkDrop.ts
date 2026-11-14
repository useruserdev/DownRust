// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {notify} from '../lib/notify';
import {useUi} from '../state/ui';
import {asHttpUrl} from './useKeyboardShortcuts';
import {isEditableTarget} from '../lib/keyboard';
import {linkTokenCount} from '../lib/urlPattern';
import {t} from '../i18n';

const LINK_TYPES = ['text/uri-list', 'text/plain'];

function carriesLink(event: DragEvent): boolean {
  const types = Array.from(event.dataTransfer?.types ?? []);
  return types.some(type => LINK_TYPES.includes(type));
}

/** Reads the link from a drop. A uri-list wins; comment lines (starting with #) are skipped. */
function readDroppedLink(data: DataTransfer): string | null {
  const uriLines = data
    .getData('text/uri-list')
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => line !== '' && !line.startsWith('#'));
  return asHttpUrl(uriLines[0] ?? null) ?? asHttpUrl(data.getData('text/plain'));
}

/**
 * Watches the whole window for a link dragged over it. Returns true while a link hovers over the window.
 * A dropped http or https link opens the Add download dialog with the address filled in.
 */
export function useLinkDrop(): boolean {
  const [isDragging, setDragging] = useState(false);

  useEffect(() => {
    let depth = 0;

    const onDragEnter = (event: DragEvent) => {
      if (!carriesLink(event)) return;
      depth += 1;
      setDragging(true);
    };

    const onDragOver = (event: DragEvent) => {
      if (!carriesLink(event) || isEditableTarget(event.target)) return;
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
    };

    const onDragLeave = (event: DragEvent) => {
      if (!carriesLink(event)) return;
      depth = Math.max(0, depth - 1);
      if (depth === 0) setDragging(false);
    };

    const onDrop = (event: DragEvent) => {
      depth = 0;
      setDragging(false);
      if (!carriesLink(event) || isEditableTarget(event.target)) return;
      event.preventDefault();
      const data = event.dataTransfer;
      if (!data) return;
      const many = [data.getData('text/uri-list'), data.getData('text/plain')].find(text => linkTokenCount(text) >= 2);
      if (many !== undefined) {
        useUi.getState().openBatch(many);
        return;
      }
      const link = readDroppedLink(data);
      if (link === null) {
        notify('error', t('errors.notLink'));
        return;
      }
      useUi.getState().openAdd({url: link});
    };

    window.addEventListener('dragenter', onDragEnter);
    window.addEventListener('dragover', onDragOver);
    window.addEventListener('dragleave', onDragLeave);
    window.addEventListener('drop', onDrop);
    return () => {
      window.removeEventListener('dragenter', onDragEnter);
      window.removeEventListener('dragover', onDragOver);
      window.removeEventListener('dragleave', onDragLeave);
      window.removeEventListener('drop', onDrop);
    };
  }, []);

  return isDragging;
}
