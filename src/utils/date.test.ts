import { describe, it, expect, beforeEach, vi } from 'vitest';
import { formatRelativeTime } from './date';

describe('formatRelativeTime', () => {
  beforeEach(() => {
    // Mock Date.now() to have consistent tests
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2024-01-15T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('recent timestamps', () => {
    it('should return "Just now" for timestamps less than 1 minute ago', () => {
      const thirtySecondsAgo = Date.now() - 30 * 1000;
      expect(formatRelativeTime(thirtySecondsAgo)).toBe('Just now');
    });

    it('should return "Just now" for current timestamp', () => {
      expect(formatRelativeTime(Date.now())).toBe('Just now');
    });
  });

  describe('minutes ago', () => {
    it('should return "1 min ago" for 1 minute ago', () => {
      const oneMinuteAgo = Date.now() - 1 * 60 * 1000;
      expect(formatRelativeTime(oneMinuteAgo)).toBe('1 min ago');
    });

    it('should return "5 mins ago" for 5 minutes ago (plural)', () => {
      const fiveMinutesAgo = Date.now() - 5 * 60 * 1000;
      expect(formatRelativeTime(fiveMinutesAgo)).toBe('5 mins ago');
    });

    it('should return "59 mins ago" for 59 minutes ago', () => {
      const fiftyNineMinutesAgo = Date.now() - 59 * 60 * 1000;
      expect(formatRelativeTime(fiftyNineMinutesAgo)).toBe('59 mins ago');
    });
  });

  describe('hours ago', () => {
    it('should return "1 hour ago" for 1 hour ago', () => {
      const oneHourAgo = Date.now() - 1 * 60 * 60 * 1000;
      expect(formatRelativeTime(oneHourAgo)).toBe('1 hour ago');
    });

    it('should return "5 hours ago" for 5 hours ago (plural)', () => {
      const fiveHoursAgo = Date.now() - 5 * 60 * 60 * 1000;
      expect(formatRelativeTime(fiveHoursAgo)).toBe('5 hours ago');
    });

    it('should return "23 hours ago" for 23 hours ago', () => {
      const twentyThreeHoursAgo = Date.now() - 23 * 60 * 60 * 1000;
      expect(formatRelativeTime(twentyThreeHoursAgo)).toBe('23 hours ago');
    });
  });

  describe('days ago', () => {
    it('should return "1 day ago" for 1 day ago', () => {
      const oneDayAgo = Date.now() - 1 * 24 * 60 * 60 * 1000;
      expect(formatRelativeTime(oneDayAgo)).toBe('1 day ago');
    });

    it('should return "3 days ago" for 3 days ago (plural)', () => {
      const threeDaysAgo = Date.now() - 3 * 24 * 60 * 60 * 1000;
      expect(formatRelativeTime(threeDaysAgo)).toBe('3 days ago');
    });

    it('should return "6 days ago" for 6 days ago', () => {
      const sixDaysAgo = Date.now() - 6 * 24 * 60 * 60 * 1000;
      expect(formatRelativeTime(sixDaysAgo)).toBe('6 days ago');
    });
  });

  describe('formatted dates', () => {
    it('should return formatted date for timestamps 7+ days ago (same year)', () => {
      const sevenDaysAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
      const result = formatRelativeTime(sevenDaysAgo);

      // Should be formatted as "Jan 8" (no year since it's 2024)
      expect(result).toMatch(/Jan 8/);
      expect(result).not.toMatch(/2024/);
    });

    it('should return formatted date with year for timestamps from previous year', () => {
      // December 15, 2023 (previous year)
      const lastYear = new Date('2023-12-15T12:00:00Z').getTime();
      const result = formatRelativeTime(lastYear);

      // Should include year since it's different from current year
      expect(result).toMatch(/Dec 15/);
      expect(result).toMatch(/2023/);
    });

    it('should return formatted date for very old timestamps', () => {
      const veryOld = new Date('2020-06-20T12:00:00Z').getTime();
      const result = formatRelativeTime(veryOld);

      expect(result).toMatch(/Jun 20/);
      expect(result).toMatch(/2020/);
    });
  });

  describe('edge cases', () => {
    it('should handle timestamp of 0 (epoch)', () => {
      const result = formatRelativeTime(0);
      // Should return formatted date from 1970
      expect(result).toMatch(/1970/);
    });

    it('should handle future timestamps gracefully', () => {
      const future = Date.now() + 1000 * 60 * 60; // 1 hour in future
      const result = formatRelativeTime(future);

      // Negative diff will result in "Just now" due to Math.floor
      expect(result).toBe('Just now');
    });
  });
});
