import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  loadHistory,
  saveToHistory,
  deleteHistoryEntry,
  clearHistory,
  getHistoryEntry,
  calculateMetadata,
  type HistoryEntry,
} from './historyStorage';
import type { CombinedReport } from '../../utils/types';

/**
 * Test Suite: History Storage Utilities
 *
 * Purpose:
 * Tests localStorage-based persistence for test result history with a 50-item limit.
 * History entries store complete test reports with metadata (counts, timestamps, upload mode).
 *
 * What This Tests:
 * - Loading/saving history entries to/from localStorage
 * - CRUD operations (Create, Read, Update, Delete)
 * - Error handling (corrupted data, quota exceeded, invalid format)
 * - Metadata calculation (counting failed/passed tests correctly)
 * - History size limits (max 50 entries)
 *
 * Key Business Logic:
 * 1. Each history entry = { id, timestamp, report, uploadMode, metadata }
 * 2. Metadata counts tests correctly (no double-counting on status changes)
 * 3. Most recent entries first (LIFO stack behavior)
 * 4. Automatic cleanup when exceeding 50 entries
 * 5. Graceful degradation on storage errors
 */
describe('historyStorage', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  describe('loadHistory', () => {
    it('should return empty array when no history exists', () => {
      const history = loadHistory();
      expect(history).toEqual([]);
    });

    it('should load history from localStorage', () => {
      const mockHistory: HistoryEntry[] = [
        {
          id: '1',
          timestamp: Date.now(),
          report: { processedAt: '2024-01-01', comparison: [] } as CombinedReport,
          uploadMode: 'single',
          metadata: { totalTests: 5, failedTests: 2, passedTests: 3 },
        },
      ];

      localStorage.setItem('playwright-test-history', JSON.stringify(mockHistory));

      const history = loadHistory();
      expect(history).toEqual(mockHistory);
    });

    it('should handle corrupted localStorage data gracefully', () => {
      localStorage.setItem('playwright-test-history', 'invalid json');

      const history = loadHistory();
      expect(history).toEqual([]);
    });

    it('should handle non-array data in localStorage', () => {
      localStorage.setItem('playwright-test-history', JSON.stringify({ foo: 'bar' }));

      const history = loadHistory();
      expect(history).toEqual([]);
    });
  });

  describe('saveToHistory', () => {
    it('should save a new entry to history', () => {
      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [],
      };

      const entry = saveToHistory({
        report,
        uploadMode: 'single',
        metadata: { totalTests: 5, failedTests: 2, passedTests: 3 },
      });

      expect(entry.id).toBeDefined();
      expect(entry.timestamp).toBeDefined();
      expect(entry.report).toEqual(report);
      expect(entry.uploadMode).toBe('single');

      const history = loadHistory();
      expect(history).toHaveLength(1);
      expect(history[0]).toEqual(entry);
    });

    it('should prepend new entries to the beginning of history', () => {
      const report1: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [],
      };

      const report2: CombinedReport = {
        processedAt: '2024-01-02',
        comparison: [],
      };

      const entry1 = saveToHistory({
        report: report1,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      const entry2 = saveToHistory({
        report: report2,
        uploadMode: 'multiple',
        metadata: { totalTests: 2, failedTests: 1, passedTests: 1 },
      });

      const history = loadHistory();
      expect(history[0]).toEqual(entry2); // Most recent first
      expect(history[1]).toEqual(entry1);
    });

    it('should limit history to MAX_HISTORY_ITEMS (50)', () => {
      // Create 51 entries
      for (let i = 0; i < 51; i++) {
        saveToHistory({
          report: { processedAt: `2024-01-${i}`, comparison: [] } as CombinedReport,
          uploadMode: 'single',
          metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
        });
      }

      const history = loadHistory();
      expect(history).toHaveLength(50);
    });

    it('should handle quota exceeded error gracefully', () => {
      // Mock localStorage.setItem to throw QuotaExceededError
      const setItemSpy = vi.spyOn(Storage.prototype, 'setItem');

      setItemSpy.mockImplementationOnce(() => {
        const error = new DOMException('QuotaExceededError');
        throw error;
      });

      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [],
      };

      const entry = saveToHistory({
        report,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      expect(entry).toBeDefined();
      setItemSpy.mockRestore();
    });
  });

  describe('deleteHistoryEntry', () => {
    it('should delete a specific entry by id', () => {
      const entry1 = saveToHistory({
        report: { processedAt: '2024-01-01', comparison: [] } as CombinedReport,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      const entry2 = saveToHistory({
        report: { processedAt: '2024-01-02', comparison: [] } as CombinedReport,
        uploadMode: 'single',
        metadata: { totalTests: 2, failedTests: 1, passedTests: 1 },
      });

      deleteHistoryEntry(entry1.id);

      const history = loadHistory();
      expect(history).toHaveLength(1);
      expect(history[0].id).toBe(entry2.id);
    });

    it('should do nothing if entry id does not exist', () => {
      saveToHistory({
        report: { processedAt: '2024-01-01', comparison: [] } as CombinedReport,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      deleteHistoryEntry('non-existent-id');

      const history = loadHistory();
      expect(history).toHaveLength(1);
    });
  });

  describe('clearHistory', () => {
    it('should remove all history entries', () => {
      saveToHistory({
        report: { processedAt: '2024-01-01', comparison: [] } as CombinedReport,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      saveToHistory({
        report: { processedAt: '2024-01-02', comparison: [] } as CombinedReport,
        uploadMode: 'multiple',
        metadata: { totalTests: 2, failedTests: 1, passedTests: 1 },
      });

      clearHistory();

      const history = loadHistory();
      expect(history).toEqual([]);
    });
  });

  describe('getHistoryEntry', () => {
    it('should return a specific entry by id', () => {
      const entry = saveToHistory({
        report: { processedAt: '2024-01-01', comparison: [] } as CombinedReport,
        uploadMode: 'single',
        metadata: { totalTests: 1, failedTests: 0, passedTests: 1 },
      });

      const retrieved = getHistoryEntry(entry.id);
      expect(retrieved).toEqual(entry);
    });

    it('should return null if entry does not exist', () => {
      const retrieved = getHistoryEntry('non-existent-id');
      expect(retrieved).toBeNull();
    });
  });

  describe('calculateMetadata', () => {
    /**
     * CRITICAL: Test Count Calculation Logic
     *
     * The key insight: comparison.length = number of test comparisons
     * Each ComparisonDetail represents ONE test, which can have:
     * - details.failure (test failed in this run)
     * - details.passedTest (test passed in this run)
     * - Both (test changed status between two file comparison)
     *
     * FIXED BUG: Previously counted both failure and passedTest separately,
     * causing double-counting. Now uses if/else if to count each test once.
     *
     * Total should ALWAYS equal comparison.length (not passed + failed)
     */
    it('should calculate correct metadata for a report with failures', () => {
      const report: CombinedReport = {
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
              failure: { testId: '2', callId: '2' } as any,
            },
          },
        ],
      };

      const metadata = calculateMetadata(report);
      // 2 comparisons = 2 total tests, both failed
      expect(metadata).toEqual({
        totalTests: 2,
        failedTests: 2,
        passedTests: 0,
      });
    });

    it('should calculate correct metadata for a report with passed tests', () => {
      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [
          {
            callId: '1',
            details: {
              passedTest: { testId: '1', callId: '1' } as any,
            },
          },
          {
            callId: '2',
            details: {
              passedTest: { testId: '2', callId: '2' } as any,
            },
          },
        ],
      };

      const metadata = calculateMetadata(report);
      expect(metadata).toEqual({
        totalTests: 2,
        failedTests: 0,
        passedTests: 2,
      });
    });

    it('should calculate correct metadata for mixed results', () => {
      // Common scenario: some tests pass, some fail
      // Each comparison detail is ONE test, categorized by if/else if
      const report: CombinedReport = {
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
        ],
      };

      const metadata = calculateMetadata(report);
      // 2 comparisons = 2 total (1 failed, 1 passed)
      expect(metadata).toEqual({
        totalTests: 2,
        failedTests: 1,
        passedTests: 1,
      });
    });

    it('should include other tests in total count', () => {
      /**
       * The "other" array contains additional test results for the same callId
       * This happens when a test has multiple executions/retries
       * These add to totalTests but not to passed/failed counts
       */
      const report: CombinedReport = {
        processedAt: '2024-01-01',
        comparison: [
          {
            callId: '1',
            details: {
              failure: { testId: '1', callId: '1' } as any,
            },
            other: [
              { testId: '2', callId: '2' } as any,
              { testId: '3', callId: '3' } as any,
            ],
          },
        ],
      };

      const metadata = calculateMetadata(report);
      expect(metadata.totalTests).toBe(3); // 1 failure + 2 other
    });
  });
});
