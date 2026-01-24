/**
 * OutputPane component - displays iteration content with virtual scrolling
 */

import { useEffect, useRef } from 'react';
import type { CSSProperties, ReactElement } from 'react';
import { List, type ListImperativeAPI } from 'react-window';
import type { OutputLine } from '@/lib/api';
import { cn } from '@/lib/utils';

interface OutputPaneProps {
  lines: OutputLine[];
  searchQuery?: string;
  scrollToLine?: number;
}

const lineTypeStyles: Record<string, string> = {
  text: '',
  tool_call: 'bg-blue-50 dark:bg-blue-950 border-l-2 border-blue-400',
  tool_result: 'bg-green-50 dark:bg-green-950 border-l-2 border-green-400',
  error: 'bg-red-50 dark:bg-red-950 border-l-2 border-red-400',
};

function highlightText(text: string, query: string): React.ReactNode {
  if (!query) return text;

  const parts = text.split(new RegExp(`(${escapeRegExp(query)})`, 'gi'));
  return parts.map((part, i) =>
    part.toLowerCase() === query.toLowerCase() ? (
      <mark key={i} className="bg-yellow-200 dark:bg-yellow-800">
        {part}
      </mark>
    ) : (
      part
    )
  );
}

function escapeRegExp(string: string): string {
  return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

interface RowCustomProps {
  lines: OutputLine[];
  searchQuery?: string;
}

function RowComponent({
  index,
  style,
  lines,
  searchQuery
}: {
  index: number;
  style: CSSProperties;
} & RowCustomProps): ReactElement {
  const line = lines[index];
  const lineTypeClass = lineTypeStyles[line.line_type] || '';

  return (
    <div
      style={style}
      className={cn(
        'px-4 py-1 font-mono text-sm flex items-start gap-2 hover:bg-accent/50',
        lineTypeClass
      )}
    >
      <span className="text-muted-foreground select-none w-10 text-right shrink-0">
        {index + 1}
      </span>
      <span className="whitespace-pre-wrap break-all">
        {highlightText(line.text, searchQuery || '')}
      </span>
    </div>
  );
}

export function OutputPane({
  lines,
  searchQuery,
  scrollToLine,
}: OutputPaneProps) {
  const listRef = useRef<ListImperativeAPI>(null);

  useEffect(() => {
    if (scrollToLine !== undefined && listRef.current) {
      listRef.current.scrollToRow({ index: scrollToLine, align: 'center' });
    }
  }, [scrollToLine]);

  if (lines.length === 0) {
    return (
      <div
        className="flex items-center justify-center h-full text-muted-foreground"
        data-testid="output-pane-empty"
      >
        No output for this iteration
      </div>
    );
  }

  return (
    <div
      className="h-full w-full"
      data-testid="output-pane"
    >
      <List<RowCustomProps>
        listRef={listRef}
        defaultHeight={500}
        rowCount={lines.length}
        rowHeight={24}
        rowComponent={RowComponent}
        rowProps={{ lines, searchQuery }}
        overscanCount={10}
        style={{ height: 500, width: '100%' }}
      />
    </div>
  );
}
