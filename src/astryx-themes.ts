// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Built by `astryx theme` — do not edit manually.
 * @astryx-theme-record {"themes":{"downrust":"./src/themes"},"defaultThemeSlug":"downrust"}
 */

import {downrustTheme as _astryxTheme0} from "./themes/downrust/downrust.js";
import "./themes/downrust/downrust.css";

export const themes = {
  "downrust": _astryxTheme0,
} as const;
export type ThemeSlug = keyof typeof themes;

export const defaultThemeSlug: keyof typeof themes = "downrust";
