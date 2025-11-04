import type { CombinedReport } from '../../utils/types';

const STORAGE_KEY = 'playwright-test-history';
const MAX_HISTORY_ITEMS = 50;

export interface HistoryEntry {
  id: string;
  timestamp: number;
  report: CombinedReport;
  uploadMode: 'single' | 'multiple';
  metadata: {
    totalTests: number;
    failedTests: number;
    passedTests: number;
  };
}

/**
 * Load test history from localStorage
 */
export function loadHistory(): HistoryEntry[] {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return [];

    const parsed = JSON.parse(stored);
    return Array.isArray(parsed) ? parsed : [];
  } catch (error) {
    console.error('Failed to load history:', error);
    return [];
  }
}

/**
 * Save a new test result to history
 */
export function saveToHistory(entry: Omit<HistoryEntry, 'id' | 'timestamp'>): HistoryEntry {
  const history = loadHistory();

  const newEntry: HistoryEntry = {
    ...entry,
    id: crypto.randomUUID(),
    timestamp: Date.now(),
  };

  // Add to beginning of array (most recent first)
  const updatedHistory = [newEntry, ...history].slice(0, MAX_HISTORY_ITEMS);

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(updatedHistory));
  } catch (error) {
    console.error('Failed to save history:', error);
    // If storage is full, try removing old items
    if (error instanceof DOMException && error.name === 'QuotaExceededError') {
      const reducedHistory = [newEntry, ...history].slice(0, Math.floor(MAX_HISTORY_ITEMS / 2));
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(reducedHistory));
      } catch (retryError) {
        console.error('Failed to save history after retry:', retryError);
      }
    }
  }

  return newEntry;
}

/**
 * Delete a specific history entry
 */
export function deleteHistoryEntry(id: string): void {
  const history = loadHistory();
  const filtered = history.filter(entry => entry.id !== id);

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(filtered));
  } catch (error) {
    console.error('Failed to delete history entry:', error);
  }
}

/**
 * Clear all history
 */
export function clearHistory(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    console.error('Failed to clear history:', error);
  }
}

/**
 * Get a specific history entry by ID
 */
export function getHistoryEntry(id: string): HistoryEntry | null {
  const history = loadHistory();
  return history.find(entry => entry.id === id) || null;
}

/**
 * Calculate metadata for a report
 */
export function calculateMetadata(report: CombinedReport): HistoryEntry['metadata'] {
  let totalTests = 0;
  let failedTests = 0;
  let passedTests = 0;

  report.comparison.forEach(detail => {
    totalTests++; // Each comparison detail is one test

    if (detail.details.failure) {
      failedTests++;
    } else if (detail.details.passedTest) {
      passedTests++;
    }

    if (detail.other && detail.other.length > 0) {
      totalTests += detail.other.length;
    }
  });

  return { totalTests, failedTests, passedTests };
}
