// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {colorVars, radiusVars, sizeVars} from '@astryxdesign/core/theme/tokens.stylex';
import type {SegmentView} from '../ipc/types';
import {threadStyleFor} from './threadColors';

const W = 1000;
const H = 24;
const HEAD_WIDTH = 3;

const styles = stylex.create({
  svg: {
    display: 'block',
    width: '100%',
    height: sizeVars['--size-element-sm'],
    borderRadius: radiusVars['--radius-inner'],
    overflow: 'hidden',
  },
  track: {fill: colorVars['--color-track']},
  settled: {fill: colorVars['--color-accent']},
  idle: {opacity: 0.55},
  head: {fill: colorVars['--color-text-primary']},
});

interface SegmentBarProps {
  segments: SegmentView[];
  /** Total size in bytes, or null when the server did not report one. */
  total: number | null;
  label: string;
}

/**
 * Connection map: one coloured band per segment across the file.
 * Active segments show a head at the end of their filled part.
 * Settled segments are drawn in the accent colour.
 * When the total is unknown, only the empty track is drawn.
 */
export function SegmentBar({segments, total, label}: SegmentBarProps) {
  const known = total !== null && total > 0;
  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      shapeRendering="crispEdges"
      role="img"
      aria-label={label}
      {...stylex.props(styles.svg)}
    >
      <rect x={0} y={0} width={W} height={H} {...stylex.props(styles.track)} />
      {known
        ? segments.map((segment, index) => {
            const length = segment.end - segment.start;
            if (length <= 0) return null;
            const x = (segment.start / total) * W;
            const width = (length / total) * W;
            const filled = Math.min(width, Math.max(0, (segment.done / length) * width));
            if (filled <= 0) return null;
            const settled = segment.done >= length;
            const fillStyle = segment.active
              ? threadStyleFor(index)
              : settled
                ? styles.settled
                : [threadStyleFor(index), styles.idle];
            const headX = Math.max(x, x + filled - HEAD_WIDTH);
            return (
              <g key={segment.start}>
                <rect x={x} y={0} width={filled} height={H} {...stylex.props(fillStyle)} />
                {segment.active ? (
                  <rect
                    x={headX}
                    y={0}
                    width={x + filled - headX}
                    height={H}
                    {...stylex.props(styles.head)}
                  />
                ) : null}
              </g>
            );
          })
        : null}
    </svg>
  );
}
