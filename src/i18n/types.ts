// SPDX-License-Identifier: GPL-3.0-or-later
import type {en} from './en';

/** A plural leaf: Russian uses all four forms, English uses one and other. */
export interface PluralForms {
  one: string;
  few?: string;
  many?: string;
  other: string;
}

type Shape<T> = T extends string
  ? string
  : T extends {readonly one: string; readonly other: string}
    ? PluralForms
    : {[K in keyof T]: Shape<T[K]>};

/** Shape every language dictionary must have. Derived from the English source. */
export type Messages = Shape<typeof en>;

type Leaves<T, Prefix extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? `${Prefix}${K}`
    : T[K] extends PluralForms
      ? `${Prefix}${K}`
      : Leaves<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

/** Every dotted key of the dictionary, for example `inspector.checksum.ok`. */
export type MessageKey = Leaves<Messages>;
