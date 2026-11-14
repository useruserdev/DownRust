// SPDX-License-Identifier: GPL-3.0-or-later
// Portions adapted from Astryx templates, Copyright (c) Meta Platforms, Inc. and affiliates (MIT); see THIRD_PARTY_NOTICES.md
import type {ReactNode} from 'react';
import {AppShell} from '@astryxdesign/core/AppShell';
import {Icon} from '@astryxdesign/core/Icon';
import {Overlay} from '@astryxdesign/core/Overlay';
import {VStack} from '@astryxdesign/core/Layout';
import {Text} from '@astryxdesign/core/Text';
import {Link2} from 'lucide-react';
import {t} from '../i18n';
import {SideNavigation} from './SideNavigation';
import {useLinkDrop} from './useLinkDrop';

/** Shown over the whole window while a link is dragged onto it. */
function DropHint() {
  return (
    <VStack gap={2} hAlign="center">
      <Icon icon={Link2} size="lg" />
      <Text weight="medium" color="inherit">
        {t('app.dropHint')}
      </Text>
    </VStack>
  );
}

/** Application frame: the side navigation rail beside the main content, with a drop hint for links. */
export function AppFrame({children}: {children: ReactNode}) {
  const isDragging = useLinkDrop();
  return (
    <Overlay
      showOn="always"
      scrim={isDragging ? 'dark' : false}
      position="fill"
      align="center"
      content={isDragging ? <DropHint /> : null}>
      <AppShell contentPadding={0} height="fill" sideNav={<SideNavigation />}>
        {children}
      </AppShell>
    </Overlay>
  );
}
