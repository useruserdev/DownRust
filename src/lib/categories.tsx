// SPDX-License-Identifier: GPL-3.0-or-later
import type {LucideIcon} from 'lucide-react';
import {Archive, File, FileText, Film, Music, Package} from 'lucide-react';
import {Icon} from '@astryxdesign/core/Icon';
import type {Category} from '../ipc/types';
import {t} from '../i18n';
import type {MessageKey} from '../i18n';

export const CATEGORY_ORDER: Category[] = ['video', 'music', 'archives', 'programs', 'documents', 'other'];

export const CATEGORY_META: Record<Category, {labelKey: MessageKey; icon: LucideIcon}> = {
  video: {labelKey: 'category.video', icon: Film},
  music: {labelKey: 'category.music', icon: Music},
  archives: {labelKey: 'category.archives', icon: Archive},
  programs: {labelKey: 'category.programs', icon: Package},
  documents: {labelKey: 'category.documents', icon: FileText},
  other: {labelKey: 'category.other', icon: File},
};

/** The category name in the screen language. */
export function categoryLabel(category: Category): string {
  return t(CATEGORY_META[category].labelKey);
}

const EXTENSION_CATEGORY: Record<string, Category> = {
  mp4: 'video', mkv: 'video', avi: 'video', mov: 'video', webm: 'video',
  mp3: 'music', flac: 'music', wav: 'music', ogg: 'music', m4a: 'music',
  zip: 'archives', rar: 'archives', '7z': 'archives', tar: 'archives', gz: 'archives', iso: 'archives',
  exe: 'programs', msi: 'programs', dmg: 'programs', appimage: 'programs',
  pdf: 'documents', doc: 'documents', docx: 'documents', epub: 'documents', txt: 'documents',
};

/** Picks a category from the file name extension. Unknown extensions fall back to other. */
export function categoryForFileName(fileName: string): Category {
  const dot = fileName.lastIndexOf('.');
  if (dot < 0) return 'other';
  const extension = fileName.slice(dot + 1).toLowerCase();
  return EXTENSION_CATEGORY[extension] ?? 'other';
}

export function CategoryIcon({category, size = 'sm'}: {category: Category; size?: 'xsm' | 'sm' | 'md' | 'lg'}) {
  return <Icon icon={CATEGORY_META[category].icon} size={size} />;
}
