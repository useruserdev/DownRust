// SPDX-License-Identifier: GPL-3.0-or-later
import {Badge} from '@astryxdesign/core/Badge';
import {HStack} from '@astryxdesign/core/Layout';
import {StackItem} from '@astryxdesign/core/Stack';
import {Text} from '@astryxdesign/core/Text';
import {CategoryIcon} from '../../../lib/categories';
import type {DownloadRecord} from '../../../ipc/types';
import {kindBadgeLabel} from '../../../lib/kind';

/** File name with its category icon; the HLS/DASH badge keeps its size and the name is truncated instead. */
export function NameCell({record}: {record: DownloadRecord}) {
  const kindLabel = kindBadgeLabel(record.kind);
  return (
    <HStack gap={2} vAlign="center">
      <StackItem size="static">
        <CategoryIcon category={record.category} />
      </StackItem>
      <StackItem size="fill">
        <Text weight="medium" maxLines={1}>
          {record.fileName}
        </Text>
      </StackItem>
      {kindLabel !== null && (
        <StackItem size="static">
          <Badge variant="info" label={kindLabel} />
        </StackItem>
      )}
    </HStack>
  );
}
