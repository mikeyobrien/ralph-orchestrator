/**
 * SearchBar component - search input with results count
 */

import { useState, useCallback } from 'react';
import { Input } from '@/components/ui/input';
import { Search, X } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface SearchBarProps {
  onSearch: (query: string) => void;
  resultCount?: number;
  currentResult?: number;
  onNextResult?: () => void;
  onPrevResult?: () => void;
}

export function SearchBar({
  onSearch,
  resultCount,
  currentResult,
  onNextResult,
  onPrevResult,
}: SearchBarProps) {
  const [value, setValue] = useState('');

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = e.target.value;
      setValue(newValue);
      onSearch(newValue);
    },
    [onSearch]
  );

  const handleClear = useCallback(() => {
    setValue('');
    onSearch('');
  }, [onSearch]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        if (e.shiftKey) {
          onPrevResult?.();
        } else {
          onNextResult?.();
        }
      }
      if (e.key === 'Escape') {
        handleClear();
      }
    },
    [onNextResult, onPrevResult, handleClear]
  );

  return (
    <div className="flex items-center gap-2" data-testid="search-bar">
      <div className="relative flex-1">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          type="text"
          placeholder="Search output..."
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          className="pl-9 pr-9"
        />
        {value && (
          <Button
            variant="ghost"
            size="sm"
            className="absolute right-1 top-1/2 -translate-y-1/2 h-6 w-6 p-0"
            onClick={handleClear}
            aria-label="clear search"
          >
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>

      {value && resultCount !== undefined && (
        <div
          className="flex items-center gap-1 text-sm text-muted-foreground whitespace-nowrap"
          data-testid="search-results"
        >
          {resultCount === 0 ? (
            <span>No results</span>
          ) : (
            <>
              <span>
                {currentResult !== undefined ? currentResult + 1 : 1} of{' '}
                {resultCount}
              </span>
              {onPrevResult && onNextResult && (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={onPrevResult}
                    disabled={resultCount === 0}
                    aria-label="previous result"
                  >
                    <span className="text-xs">^</span>
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={onNextResult}
                    disabled={resultCount === 0}
                    aria-label="next result"
                  >
                    <span className="text-xs">v</span>
                  </Button>
                </>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
