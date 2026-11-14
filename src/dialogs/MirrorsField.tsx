// SPDX-License-Identifier: GPL-3.0-or-later
import {Collapsible} from '@astryxdesign/core/Collapsible';
import {TextArea} from '@astryxdesign/core/TextArea';
import {t} from '../i18n';

const HTTP_ADDRESS = /^https?:\/\/\S+$/i;

/** Splits the mirror text into addresses. `valid` is false when a line is not an http or https address. */
export function parseMirrors(text: string): {addresses: string[]; valid: boolean} {
  const addresses = text
    .split('\n')
    .map(line => line.trim())
    .filter(line => line !== '');
  return {addresses, valid: addresses.every(address => HTTP_ADDRESS.test(address))};
}

interface MirrorsFieldProps {
  value: string;
  onChange: (value: string) => void;
}

/** Other addresses of the same file, one per line. Collapsed by default. */
export function MirrorsField({value, onChange}: MirrorsFieldProps) {
  const {valid} = parseMirrors(value);
  return (
    <Collapsible trigger={t('mirrors.title')} defaultIsOpen={false}>
      <TextArea
        label={t('mirrors.label')}
        description={t('mirrors.hint')}
        value={value}
        onChange={onChange}
        placeholder="https://"
        rows={3}
        status={valid ? undefined : {type: 'error', message: t('mirrors.invalid')}}
      />
    </Collapsible>
  );
}
