import { useState, useCallback, useEffect, useMemo } from 'react';
import type { CombinedReport } from '../../utils/types';
import {
  loadHistory,
  saveToHistory,
  deleteHistoryEntry,
  clearHistory,
  calculateMetadata,
  type HistoryEntry,
} from '../utils/historyStorage';

export type FilterStatus = 'all' | 'passed' | 'failed';

export interface UseTestHistoryReturn {
  history: HistoryEntry[];
  currentFilter: FilterStatus;
  setCurrentFilter: (filter: FilterStatus) => void;
  filteredResults: CombinedReport | null;
  addToHistory: (report: CombinedReport, uploadMode: 'single' | 'multiple') => void;
  deleteEntry: (id: string) => void;
  clearAllHistory: () => void;
  refreshHistory: () => void;
}

/**
 * Custom hook for managing test result history with filtering
 * Uses React 19 best practices including useCallback and useMemo for performance
 */
export function useTestHistory(currentResults: CombinedReport | null): UseTestHistoryReturn {
  const [history, setHistory] = useState<HistoryEntry[]>(() => loadHistory());
  const [currentFilter, setCurrentFilter] = useState<FilterStatus>('all');

  /**
   * Filter current results based on selected filter
   * Uses useMemo to avoid recomputation on every render (React 19 best practice)
   */
  const filteredResults = useMemo((): CombinedReport | null => {
    if (!currentResults) return null;
    if (currentFilter === 'all') return currentResults;

    const filtered = currentResults.comparison.filter(detail => {
      if (currentFilter === 'failed') {
        return !!detail.details.failure;
      }
      if (currentFilter === 'passed') {
        return !!detail.details.passedTest;
      }
      return true;
    });

    return {
      ...currentResults,
      comparison: filtered,
    };
  }, [currentResults, currentFilter]);

  /**
   * Refresh history from localStorage
   * Uses useCallback to maintain referential equality (React 19 best practice)
   */
  const refreshHistory = useCallback(() => {
    setHistory(loadHistory());
  }, []);

  /**
   * Add current results to history
   */
  const addToHistory = useCallback((report: CombinedReport, uploadMode: 'single' | 'multiple') => {
    const metadata = calculateMetadata(report);
    const newEntry = saveToHistory({ report, uploadMode, metadata });
    setHistory(prev => [newEntry, ...prev.filter(e => e.id !== newEntry.id)]);
  }, []);

  /**
   * Delete a history entry
   */
  const deleteEntry = useCallback((id: string) => {
    deleteHistoryEntry(id);
    setHistory(prev => prev.filter(entry => entry.id !== id));
  }, []);

  /**
   * Clear all history
   */
  const clearAllHistory = useCallback(() => {
    clearHistory();
    setHistory([]);
  }, []);

  // Sync with localStorage changes from other tabs/windows
  useEffect(() => {
    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === 'playwright-test-history') {
        refreshHistory();
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, [refreshHistory]);

  return {
    history,
    currentFilter,
    setCurrentFilter,
    filteredResults,
    addToHistory,
    deleteEntry,
    clearAllHistory,
    refreshHistory,
  };
}
