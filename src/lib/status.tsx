// SPDX-License-Identifier: GPL-3.0-or-later
import {StatusDot} from '@astryxdesign/core/StatusDot';
import type {StatusDotVariant} from '@astryxdesign/core/StatusDot';
import type {DownloadStatus} from '../ipc/types';
import {t} from '../i18n';
import type {MessageKey} from '../i18n';

const STATUS_LABEL: Record<DownloadStatus, MessageKey> = {
  queued: 'status.queued',
  downloading: 'status.downloading',
  paused: 'status.paused',
  completed: 'status.completed',
  error: 'status.error',
};

const STATUS_DOT: Record<DownloadStatus, StatusDotVariant> = {
  queued: 'neutral',
  downloading: 'accent',
  paused: 'warning',
  completed: 'success',
  error: 'error',
};

export function statusLabel(status: DownloadStatus): string {
  return t(STATUS_LABEL[status]);
}

export function statusDotVariant(status: DownloadStatus): StatusDotVariant {
  return STATUS_DOT[status];
}

export function DownloadStatusDot({status}: {status: DownloadStatus}) {
  return <StatusDot variant={statusDotVariant(status)} label={statusLabel(status)} />;
}
