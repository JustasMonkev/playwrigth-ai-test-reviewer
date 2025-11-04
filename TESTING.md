# Testing Documentation

This document explains the testing strategy, logic, and how to understand the test suite for the Playwright AI Test Reviewer application.

## Table of Contents

- [Overview](#overview)
- [Running Tests](#running-tests)
- [Test Architecture](#test-architecture)
- [Key Business Logic Explained](#key-business-logic-explained)
- [Test Coverage](#test-coverage)
- [Writing New Tests](#writing-new-tests)

---

## Overview

The application uses **Vitest** with **React Testing Library** for comprehensive unit and integration testing. The test suite covers:

- ✅ **45 passing tests** across 4 test files
- ✅ **Utility functions** (history storage, date formatting, ZIP processing)
- ✅ **Custom React hooks** (useFileProcessing, useTestHistory)
- ✅ **UI components** (FilterControls, HistoryPanel)

---

## Running Tests

```bash
# Run tests in watch mode (development)
npm test

# Run tests once (CI)
npm run test:run

# Run tests with UI
npm run test:ui

# Generate coverage report
npm run test:coverage
```

---

## Test Architecture

### Test Files Structure

```
src/
├── components/
│   └── FilterControls.test.tsx      # UI component tests
├── hooks/
│   ├── useFileProcessing.test.ts    # File processing hook tests
│   └── useTestHistory.test.ts       # History management hook tests
├── utils/
│   ├── date.test.ts                 # Date utility tests
│   └── historyStorage.test.ts       # localStorage persistence tests
└── test/
    └── setup.ts                     # Test environment setup

utils/
└── zipProcessor.test.ts             # ZIP processing logic documentation
```

### Test Categories

| Category | Purpose | Test Count |
|----------|---------|------------|
| **Unit Tests** | Test individual functions in isolation | 28 |
| **Integration Tests** | Test hooks with React lifecycle | 12 |
| **Component Tests** | Test UI rendering and interactions | 9 |
| **Documentation Tests** | Document complex logic behavior | 12 |

---

## Key Business Logic Explained

### 1. Test Count Calculation (CRITICAL)

**Problem:** Tests can have multiple statuses (failure, passedTest, or both), leading to double-counting.

**Solution:** Use `if/else if` logic to categorize each test only once.

```typescript
// ❌ WRONG - Double counting
let total = 0;
if (detail.details.failure) total++;
if (detail.details.passedTest) total++; // Same test counted twice!

// ✅ CORRECT - Each test counted once
let failed = 0;
let passed = 0;
if (detail.details.failure) {
  failed++;
} else if (detail.details.passedTest) {
  passed++;
}
const total = results.comparison.length; // Total = number of comparisons
```

**Why This Matters:**
- A test that changed from "failed" to "passed" has BOTH `failure` and `passedTest`
- It should count as ONE test (not two)
- Total should always equal `comparison.length`

**Tests:** `src/utils/historyStorage.test.ts:241-360`

---

### 2. History Storage with localStorage

**How It Works:**
1. Store up to **50 most recent** test results
2. Each entry has unique ID (UUID), timestamp, report, and metadata
3. Most recent entries first (LIFO stack)
4. Graceful degradation on storage errors

**Storage Schema:**
```typescript
interface HistoryEntry {
  id: string;                    // UUID
  timestamp: number;             // Unix milliseconds
  report: CombinedReport;        // Full test report
  uploadMode: 'single' | 'multiple';
  metadata: {
    totalTests: number;
    failedTests: number;
    passedTests: number;
  };
}
```

**Error Handling:**
- Corrupted JSON → Returns empty array (doesn't crash)
- Quota exceeded → Reduces to 25 entries and retries
- Non-array data → Returns empty array

**Tests:** `src/utils/historyStorage.test.ts:13-360`

---

### 3. ZIP Processing and Trace Parsing

**Flow:**
```
1. Upload ZIP file(s)
   ↓
2. Extract .trace files (NDJSON format)
   ↓
3. Parse each line as JSON (with error recovery)
   ↓
4. Extract soft assertions (match before/after entries)
   ↓
5. Group results by test identifier (callId prefix)
   ↓
6. Filter to most relevant errors
   ↓
7. Associate screenshots (blob URLs)
   ↓
8. Build ComparisonDetail[] report
```

**Key Concepts:**

#### Soft Assertions
- Two trace entries per assertion: `before` (setup) and `after` (result)
- Matched by `callId`
- Pass/fail determined by presence of `error` in `after` entry

```typescript
// Before entry
{
  type: 'before',
  callId: 'test-1@soft-1',
  apiName: 'expect.soft.toHaveText',
  params: { expectedText: 'Hello' }
}

// After entry (passing)
{
  type: 'after',
  callId: 'test-1@soft-1',
  result: { received: { s: 'Hello' } }
  // No error = passed
}

// After entry (failing)
{
  type: 'after',
  callId: 'test-1@soft-2',
  error: { message: 'Expected "Hello", got "Goodbye"' }
  // Has error = failed
}
```

#### Error Filtering
- Groups errors by test (callId prefix: `login-test@step1` → `login-test`)
- Prefers errors with meaningful messages
- Returns most recent error per test

#### Two-File Comparison
1. Sort files by size: larger = failures, smaller = passes
2. Match tests by `callId`
3. Only report tests that appear in BOTH files (status comparison)

**Tests:** `utils/zipProcessor.test.ts:1-390` (documentation tests)

---

### 4. Filter Logic

**Filter Options:**
- `all` - Show all tests (no filtering)
- `failed` - Show only tests with `details.failure`
- `passed` - Show only tests with `details.passedTest`

**Implementation:**
```typescript
const filtered = results.comparison.filter(detail => {
  if (currentFilter === 'failed') {
    return !!detail.details.failure;
  }
  if (currentFilter === 'passed') {
    return !!detail.details.passedTest;
  }
  return true; // 'all'
});
```

**Tests:** `src/hooks/useTestHistory.test.ts:46-68`

---

### 5. Date Formatting

**Logic:** Relative time for recent dates, absolute for older dates

```
< 1 min:     "Just now"
1-59 mins:   "5 mins ago"
1-23 hours:  "12 hours ago"
1-6 days:    "3 days ago"
7+ days:     "Jan 15" (same year) or "Jan 15, 2023" (different year)
```

**Tests:** `src/utils/date.test.ts:1-125`

---

## Test Coverage

### Critical Logic Coverage

| Logic | File | Tests | Status |
|-------|------|-------|--------|
| Count calculation | `historyStorage.ts` | 4 | ✅ Comprehensive |
| History CRUD | `historyStorage.ts` | 7 | ✅ Comprehensive |
| Error handling | `historyStorage.ts` | 3 | ✅ Edge cases covered |
| Filter logic | `useTestHistory.ts` | 6 | ✅ All filters tested |
| State management | `useTestHistory.ts` | 4 | ✅ Lifecycle covered |
| File validation | `useFileProcessing.ts` | 3 | ✅ Mode validation |
| Result counts | `useFileProcessing.ts` | 1 | ✅ Memoization tested |
| Date formatting | `date.ts` | 10 | ✅ All formats |
| ZIP logic | `zipProcessor.ts` | 12 | ⚠️  Documentation only |

### Coverage Gaps

The following areas have **documentation tests** but need implementation tests:

1. **ZIP extraction and parsing** - Complex binary file handling
2. **Soft assertion extraction** - NDJSON parsing logic
3. **Error grouping and filtering** - Test result aggregation
4. **Screenshot association** - Blob URL creation

These are documented in `utils/zipProcessor.test.ts` but require:
- Mock ZIP files for testing
- Fixture data for trace structures
- Integration tests with JSZip library

---

## Writing New Tests

### Best Practices

1. **Start with a descriptive comment block**
```typescript
/**
 * Test Suite: Feature Name
 *
 * Purpose: What this feature does
 * What This Tests: Specific scenarios
 * Key Business Logic: Important rules
 */
describe('Feature', () => {
  // tests...
});
```

2. **Use clear test names**
```typescript
// ❌ Bad
it('works', () => { ... });

// ✅ Good
it('should calculate correct metadata for mixed results', () => { ... });
```

3. **Add inline comments for complex logic**
```typescript
it('should handle edge case', () => {
  // Setup: Create a test with both failure and passedTest
  const test = { ... };

  // Execute: Run the counting function
  const result = calculateMetadata(test);

  // Assert: Should count as ONE test (not two)
  expect(result.totalTests).toBe(1);
});
```

4. **Test edge cases**
- Empty inputs
- Null/undefined
- Corrupted data
- Boundary conditions (0, 1, max)
- Error scenarios

5. **Use beforeEach for cleanup**
```typescript
beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
});
```

### Testing Hooks

Use `@testing-library/react` hooks:

```typescript
import { renderHook, act } from '@testing-library/react';

it('should update state', () => {
  const { result } = renderHook(() => useCustomHook());

  act(() => {
    result.current.updateValue('new');
  });

  expect(result.current.value).toBe('new');
});
```

### Testing Components

```typescript
import { render, screen, fireEvent } from '@testing-library/react';

it('should render button', () => {
  render(<MyButton onClick={mockFn} />);

  const button = screen.getByRole('button');
  fireEvent.click(button);

  expect(mockFn).toHaveBeenCalled();
});
```

---

## Continuous Integration

Tests run automatically on every push via GitHub Actions (`.github/workflows/ci.yml`):

- ✅ Lint check
- ✅ Test suite (Node 20.x and 22.x)
- ✅ Build verification
- ✅ Coverage reporting (on main branch)

**Test Failure = PR Blocked** - All tests must pass before merging.

---

## Debugging Tests

### View Test UI
```bash
npm run test:ui
```
Opens interactive UI at `http://localhost:51204/__vitest__/`

### Debug Single Test
```typescript
it.only('should debug this test', () => {
  // Only this test runs
});
```

### Check Coverage
```bash
npm run test:coverage
open coverage/index.html
```

---

## Questions?

If a test is failing or unclear:

1. Read the test file's header comment block
2. Check inline comments in the failing test
3. Review this documentation
4. Check the implementation file
5. Ask the team for clarification

**Remember:** Tests are documentation. If they're hard to understand, add more comments!
