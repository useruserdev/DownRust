// SPDX-License-Identifier: GPL-3.0-or-later
import {defineTheme} from '@astryxdesign/core/theme';
import {neutralTheme} from '@astryxdesign/theme-neutral/built';

/** Warm graphite surfaces with an orange transfer accent. */
export const downrustTheme = defineTheme({
  name: 'downrust',
  extends: neutralTheme,
  color: {accent: ['#E4571A', '#F26A2E'], neutralStyle: 'warm'},
  tokens: {
    '--color-accent': ['#C9440E', '#F26A2E'],
    '--color-on-accent': ['#FFFFFF', '#1C1A19'],
    '--color-background-body': ['#F3F1EE', '#141311'],
    '--color-background-surface': ['#FAF9F7', '#1B1A18'],
    '--color-background-card': ['#FFFFFF', '#211F1D'],
    '--color-background-muted': ['#EAE7E2', '#2A2825'],
    '--color-background-popover': ['#FFFFFF', '#24221F'],
    '--color-border': ['#DCD8D2', '#34312D'],
    '--color-border-emphasized': ['#BDB7AE', '#4A4640'],
    '--color-text-primary': ['#1E1C19', '#EFECE7'],
    '--color-text-secondary': ['#5F5A53', '#A9A39A'],
    '--color-track': ['#E4E0DA', '#2E2B28'],
    '--font-family-body': "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif",
    '--font-family-heading': "'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif",
    '--font-family-code': "'Cascadia Mono', Consolas, monospace",
  },
});
