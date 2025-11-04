import { memo, useCallback, useDeferredValue } from 'react';
import type { HistoryEntry } from '../utils/historyStorage';
import { Card, CardContent, CardHeader } from './ui/Card';
import { Button } from './ui/Button';
import { Badge } from './ui/Badge';
import { History, Trash2, Clock, FileText } from 'lucide-react';

interface HistoryPanelProps {
  history: HistoryEntry[];
  onLoadHistory: (id: string) => void;
  onDeleteEntry: (id: string) => void;
  onClearAll: () => void;
  currentHistoryId?: string | null;
}

/**
 * Panel component for displaying test result history
 * Uses React 19 best practices:
 * - memo for preventing unnecessary re-renders
 * - useDeferredValue for smooth UI updates
 * - useCallback for stable function references
 */
export const HistoryPanel = memo(function HistoryPanel({
  history,
  onLoadHistory,
  onDeleteEntry,
  onClearAll,
  currentHistoryId,
}: HistoryPanelProps) {
  // Use deferred value for smooth transitions when history updates (React 19 best practice)
  const deferredHistory = useDeferredValue(history);

  const handleDelete = useCallback((e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (confirm('Are you sure you want to delete this history entry?')) {
      onDeleteEntry(id);
    }
  }, [onDeleteEntry]);

  const handleClearAll = useCallback(() => {
    if (confirm('Are you sure you want to clear all history? This action cannot be undone.')) {
      onClearAll();
    }
  }, [onClearAll]);

  const formatDate = useCallback((timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins} min${diffMins > 1 ? 's' : ''} ago`;
    if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    if (diffDays < 7) return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;

    return date.toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    });
  }, []);

  if (deferredHistory.length === 0) {
    return (
      <Card className="bg-gray-50">
        <CardHeader>
          <div className="flex items-center gap-2">
            <History className="w-5 h-5 text-gray-600" />
            <h3 className="text-lg font-semibold">Test History</h3>
          </div>
        </CardHeader>
        <CardContent>
          <div className="text-center py-8 text-gray-500">
            <History className="w-12 h-12 mx-auto mb-3 opacity-30" />
            <p>No history yet</p>
            <p className="text-sm mt-1">Upload test results to start building history</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="bg-gray-50">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <History className="w-5 h-5 text-gray-600" />
            <h3 className="text-lg font-semibold">Test History</h3>
            <Badge variant="default" className="ml-2">
              {deferredHistory.length}
            </Badge>
          </div>
          {deferredHistory.length > 0 && (
            <Button
              onClick={handleClearAll}
              variant="outline"
              size="sm"
              className="text-red-600 hover:bg-red-50 hover:text-red-700"
            >
              Clear All
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        <div className="space-y-2 max-h-96 overflow-y-auto">
          {deferredHistory.map((entry) => {
            const isActive = currentHistoryId === entry.id;

            return (
              <div
                key={entry.id}
                onClick={() => onLoadHistory(entry.id)}
                className={`
                  p-3 rounded-lg border cursor-pointer transition-all
                  ${isActive
                    ? 'bg-blue-50 border-blue-300 shadow-md'
                    : 'bg-white border-gray-200 hover:border-blue-200 hover:shadow-sm'
                  }
                `}
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <FileText className="w-4 h-4 text-gray-500 flex-shrink-0" />
                      <span className="text-sm font-semibold text-gray-700 capitalize">
                        {entry.uploadMode === 'single' ? 'Single File' : 'Comparison'}
                      </span>
                      <div className="flex items-center gap-1 text-xs text-gray-500">
                        <Clock className="w-3 h-3" />
                        {formatDate(entry.timestamp)}
                      </div>
                    </div>

                    <div className="flex items-center gap-2 mt-2">
                      <span className="text-xs text-gray-600">
                        Total: {entry.metadata.totalTests}
                      </span>
                      {entry.metadata.failedTests > 0 && (
                        <Badge variant="error" className="text-xs">
                          {entry.metadata.failedTests} Failed
                        </Badge>
                      )}
                      {entry.metadata.passedTests > 0 && (
                        <Badge variant="success" className="text-xs">
                          {entry.metadata.passedTests} Passed
                        </Badge>
                      )}
                    </div>
                  </div>

                  <button
                    onClick={(e) => handleDelete(e, entry.id)}
                    className="ml-2 p-1.5 rounded hover:bg-red-100 text-gray-400 hover:text-red-600 transition-colors flex-shrink-0"
                    aria-label="Delete entry"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
});
