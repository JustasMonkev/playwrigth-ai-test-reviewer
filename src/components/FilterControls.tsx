import { memo } from 'react';
import type { FilterStatus } from '../hooks/useTestHistory';
import { Button } from './ui/Button';
import { Filter, CheckCircle2, XCircle, List } from 'lucide-react';

interface FilterControlsProps {
  currentFilter: FilterStatus;
  onFilterChange: (filter: FilterStatus) => void;
  totalCount?: number;
  passedCount?: number;
  failedCount?: number;
}

/**
 * Filter controls for test results
 * Uses React.memo for performance optimization (React 19 best practice)
 */
export const FilterControls = memo(function FilterControls({
  currentFilter,
  onFilterChange,
  totalCount = 0,
  passedCount = 0,
  failedCount = 0,
}: FilterControlsProps) {
  const filters: Array<{
    value: FilterStatus;
    label: string;
    icon: React.ReactNode;
    count: number;
  }> = [
    {
      value: 'all',
      label: 'All Tests',
      icon: <List className="w-4 h-4" />,
      count: totalCount,
    },
    {
      value: 'failed',
      label: 'Failed',
      icon: <XCircle className="w-4 h-4" />,
      count: failedCount,
    },
    {
      value: 'passed',
      label: 'Passed',
      icon: <CheckCircle2 className="w-4 h-4" />,
      count: passedCount,
    },
  ];

  return (
    <div className="bg-white border border-gray-200 rounded-lg p-4 mb-6 shadow-sm">
      <div className="flex items-center gap-2 mb-3">
        <Filter className="w-5 h-5 text-gray-600" />
        <h3 className="text-sm font-semibold text-gray-700">Filter Results</h3>
      </div>

      <div className="flex flex-wrap gap-2">
        {filters.map(filter => (
          <Button
            key={filter.value}
            onClick={() => onFilterChange(filter.value)}
            variant={currentFilter === filter.value ? 'default' : 'outline'}
            className={`
              flex items-center gap-2 transition-all
              ${currentFilter === filter.value ? 'ring-2 ring-blue-500 ring-offset-1' : ''}
            `}
          >
            {filter.icon}
            <span>{filter.label}</span>
            <span
              className={`
                ml-1 px-2 py-0.5 rounded-full text-xs font-semibold
                ${currentFilter === filter.value
                  ? 'bg-white text-blue-600'
                  : 'bg-gray-100 text-gray-600'
                }
              `}
            >
              {filter.count}
            </span>
          </Button>
        ))}
      </div>

      {currentFilter !== 'all' && (
        <div className="mt-3 text-xs text-gray-500">
          Showing {currentFilter === 'failed' ? failedCount : passedCount} of {totalCount} tests
        </div>
      )}
    </div>
  );
});
