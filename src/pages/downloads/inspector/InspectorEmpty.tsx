// SPDX-License-Identifier: GPL-3.0-or-later
import {EmptyState} from '@astryxdesign/core/EmptyState';
import {Icon} from '@astryxdesign/core/Icon';
import {MousePointerClick} from 'lucide-react';
import {t} from '../../../i18n';

/** Shown in the details panel when no download is selected. */
export function InspectorEmpty() {
  return (
    <EmptyState
      icon={<Icon icon={MousePointerClick} />}
      title={t('inspector.nothingSelected')}
      description={t('inspector.selectHint')}
      isCompact
    />
  );
}
