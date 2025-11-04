import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTestHistory } from './useTestHistory';
import { clearHistory } from '../utils/historyStorage';
import type { CombinedReport } from '../../utils/types';

describe('useTestHistory', () => {
  beforeEach(() => {
    localStorage.clear();
    clearHistory();
  });

  describe('initial state', () => {
    it('should initialize with empty history and "all" filter', () => {
      const { result } = renderHook(() => useTestHistory(null));

      expect(result.current.history).toEqual([]);
      expect(result.current.currentFilter).toBe('all');
      expect(result.current.filteredResults).toBeNull();
    });
  });

  describe('addToHistory', () => {
    it('should add a report to history', () => {
      const { result } = renderHook(() => useTestHistory(null));

      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [
          {
            callId: '1',
            details: {
              failure: { testId: '1', callId: '1' } as any,
            },
          },
        ],
      };

      act(() => {
        result.current.addToHistory(report, 'single');
      });

      expect(result.current.history).toHaveLength(1);
      expect(result.current.history[0].report).toEqual(report);
      expect(result.current.history[0].uploadMode).toBe('single');
    });
  });

  describe('filtering', () => {
    const mockReport: CombinedReport = {
      processedAt: '2024-01-01',
      comparison: [
        {
          callId: '1',
          details: {
            failure: { testId: '1', callId: '1' } as any,
          },
        },
        {
          callId: '2',
          details: {
            passedTest: { testId: '2', callId: '2' } as any,
          },
        },
        {
          callId: '3',
          details: {
            failure: { testId: '3', callId: '3' } as any,
          },
        },
      ],
    };

    it('should filter to show only failed tests', () => {
      const { result } = renderHook(() => useTestHistory(mockReport));

      act(() => {
        result.current.setCurrentFilter('failed');
      });

      expect(result.current.filteredResults?.comparison).toHaveLength(2);
      expect(result.current.filteredResults?.comparison.every(
        (c) => c.details.failure !== undefined
      )).toBe(true);
    });

    it('should filter to show only passed tests', () => {
      const { result } = renderHook(() => useTestHistory(mockReport));

      act(() => {
        result.current.setCurrentFilter('passed');
      });

      expect(result.current.filteredResults?.comparison).toHaveLength(1);
      expect(result.current.filteredResults?.comparison[0].details.passedTest).toBeDefined();
    });

    it('should show all tests when filter is "all"', () => {
      const { result } = renderHook(() => useTestHistory(mockReport));

      act(() => {
        result.current.setCurrentFilter('all');
      });

      expect(result.current.filteredResults?.comparison).toHaveLength(3);
    });

    it('should return null when no current results', () => {
      const { result } = renderHook(() => useTestHistory(null));

      act(() => {
        result.current.setCurrentFilter('failed');
      });

      expect(result.current.filteredResults).toBeNull();
    });
  });

  describe('deleteEntry', () => {
    it('should delete a history entry', () => {
      const { result } = renderHook(() => useTestHistory(null));

      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [],
      };

      act(() => {
        result.current.addToHistory(report, 'single');
      });

      const entryId = result.current.history[0].id;

      act(() => {
        result.current.deleteEntry(entryId);
      });

      expect(result.current.history).toHaveLength(0);
    });
  });

  describe('clearAllHistory', () => {
    it('should clear all history entries', () => {
      const { result } = renderHook(() => useTestHistory(null));

      act(() => {
        result.current.addToHistory(
          { processedAt: '2024-01-01', comparison: [] },
          'single'
        );
        result.current.addToHistory(
          { processedAt: '2024-01-02', comparison: [] },
          'multiple'
        );
      });

      expect(result.current.history).toHaveLength(2);

      act(() => {
        result.current.clearAllHistory();
      });

      expect(result.current.history).toHaveLength(0);
    });
  });

  describe('refreshHistory', () => {
    it('should refresh history from localStorage', () => {
      const { result } = renderHook(() => useTestHistory(null));

      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [],
      };

      // Add entry directly
      act(() => {
        result.current.addToHistory(report, 'single');
      });

      expect(result.current.history).toHaveLength(1);

      // Refresh should maintain the history
      act(() => {
        result.current.refreshHistory();
      });

      expect(result.current.history).toHaveLength(1);
    });
  });

  describe('storage event listener', () => {
    it('should respond to storage events', async () => {
      const { result } = renderHook(() => useTestHistory(null));

      // Add entry
      act(() => {
        result.current.addToHistory(
          { processedAt: '2024-01-01', comparison: [] },
          'single'
        );
      });

      expect(result.current.history).toHaveLength(1);

      // Manually clear localStorage to simulate another tab clearing
      localStorage.clear();

      // Simulate storage event from another tab
      act(() => {
        window.dispatchEvent(
          new StorageEvent('storage', {
            key: 'playwright-test-history',
            newValue: JSON.stringify([]),
            oldValue: null,
            storageArea: localStorage,
            url: 'http://localhost',
          })
        );
      });

      // History should be updated to empty
      expect(result.current.history).toEqual([]);
    });
  });
});
