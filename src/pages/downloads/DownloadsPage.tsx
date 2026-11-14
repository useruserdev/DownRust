// SPDX-License-Identifier: GPL-3.0-or-later
// Portions adapted from Astryx templates, Copyright (c) Meta Platforms, Inc. and affiliates (MIT); see THIRD_PARTY_NOTICES.md

/**
 * Downloads page, built on the Astryx incident-console frame.
 *
 *   Frame: header | live table (fill) | details panel 380 (resizable) | status footer
 *
 * Responsive contract:
 *   > 1024px  header | table | details panel
 *   <= 1024px details panel hidden; the table keeps full width
 */

import {useMediaQuery} from '@astryxdesign/core/hooks';
import {EmptyState} from '@astryxdesign/core/EmptyState';
import {Icon} from '@astryxdesign/core/Icon';
import {Layout, LayoutContent, LayoutPanel} from '@astryxdesign/core/Layout';
import {ResizeHandle, useResizable} from '@astryxdesign/core/Resizable';
import {MousePointerClick} from 'lucide-react';
import {useDownloads} from '../../state/downloads';
import {DownloadInspector} from './inspector/DownloadInspector';
import {DownloadsHeader} from './DownloadsHeader';
import {DownloadsTable} from './DownloadsTable';
import {StatusFooter} from './StatusFooter';
import {t} from '../../i18n';

/** Downloads page: header, live table, details panel and status footer. */
export function DownloadsPage() {
  const selectedId = useDownloads(state => state.selectedId);
  const selectedExists = useDownloads(state => (state.selectedId === null ? false : state.selectedId in state.records));

  // The details panel is hidden at <= 1024px so the table keeps its full width.
  const isNarrow = useMediaQuery('(max-width: 1024px)');

  const inspectorPanel = useResizable({
    defaultSize: 340,
    minSize: 300,
    maxSize: 480,
  });

  return (
    <>
      <Layout
        height="fill"
        header={<DownloadsHeader />}
        content={
          <LayoutContent padding={0}>
            <DownloadsTable />
          </LayoutContent>
        }
        end={
          isNarrow ? undefined : (
            <>
              <ResizeHandle
                direction="horizontal"
                hasDivider
                isAlwaysVisible={false}
                resizable={inspectorPanel.props}
                label={t('downloads.resizeDetails')}
              />
              <LayoutPanel width={inspectorPanel.size} padding={0} label={t('inspector.label')}>
                {selectedId !== null && selectedExists ? (
                  <DownloadInspector id={selectedId} />
                ) : (
                  <EmptyState
                    title={t('inspector.nothingSelected')}
                    description={t('inspector.selectHint')}
                    icon={<Icon icon={MousePointerClick} size="lg" />}
                    isCompact
                  />
                )}
              </LayoutPanel>
            </>
          )
        }
        footer={<StatusFooter />}
      />
    </>
  );
}
