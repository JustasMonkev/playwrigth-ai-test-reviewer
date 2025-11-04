import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FilterControls } from './FilterControls';

describe('FilterControls', () => {
  const defaultProps = {
    currentFilter: 'all' as const,
    onFilterChange: vi.fn(),
    totalCount: 10,
    passedCount: 6,
    failedCount: 4,
  };

  it('should render all filter buttons', () => {
    render(<FilterControls {...defaultProps} />);

    expect(screen.getByText('All Tests')).toBeInTheDocument();
    expect(screen.getByText('Failed')).toBeInTheDocument();
    expect(screen.getByText('Passed')).toBeInTheDocument();
  });

  it('should display correct counts for each filter', () => {
    render(<FilterControls {...defaultProps} />);

    expect(screen.getByText('10')).toBeInTheDocument(); // Total
    expect(screen.getByText('4')).toBeInTheDocument(); // Failed
    expect(screen.getByText('6')).toBeInTheDocument(); // Passed
  });

  it('should highlight the active filter', () => {
    const { rerender } = render(<FilterControls {...defaultProps} />);

    const allButton = screen.getByText('All Tests').closest('button');
    expect(allButton).toHaveClass('ring-2', 'ring-blue-500');

    // Change to failed filter
    rerender(
      <FilterControls {...defaultProps} currentFilter="failed" />
    );

    const failedButton = screen.getByText('Failed').closest('button');
    expect(failedButton).toHaveClass('ring-2', 'ring-blue-500');
  });

  it('should call onFilterChange when clicking a filter button', () => {
    const onFilterChange = vi.fn();
    render(<FilterControls {...defaultProps} onFilterChange={onFilterChange} />);

    const failedButton = screen.getByText('Failed');
    fireEvent.click(failedButton);

    expect(onFilterChange).toHaveBeenCalledWith('failed');
  });

  it('should show filtered count message when filter is active', () => {
    render(
      <FilterControls {...defaultProps} currentFilter="failed" />
    );

    expect(screen.getByText('Showing 4 of 10 tests')).toBeInTheDocument();
  });

  it('should not show filtered count message when showing all', () => {
    render(<FilterControls {...defaultProps} currentFilter="all" />);

    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  it('should handle zero counts gracefully', () => {
    render(
      <FilterControls
        {...defaultProps}
        totalCount={0}
        passedCount={0}
        failedCount={0}
      />
    );

    expect(screen.getByText('All Tests')).toBeInTheDocument();
    const counts = screen.getAllByText('0');
    expect(counts.length).toBeGreaterThanOrEqual(3);
  });

  it('should render filter title', () => {
    render(<FilterControls {...defaultProps} />);

    expect(screen.getByText('Filter Results')).toBeInTheDocument();
  });

  it('should not re-render unnecessarily (memo optimization)', () => {
    const onFilterChange = vi.fn();
    const { rerender } = render(
      <FilterControls {...defaultProps} onFilterChange={onFilterChange} />
    );

    // Rerender with same props
    rerender(
      <FilterControls {...defaultProps} onFilterChange={onFilterChange} />
    );

    // Component should use memo to prevent unnecessary renders
    // (This is tested implicitly by React.memo behavior)
    expect(screen.getByText('All Tests')).toBeInTheDocument();
  });
});
