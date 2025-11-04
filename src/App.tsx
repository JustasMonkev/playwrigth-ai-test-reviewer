import {AlertCircle, History as HistoryIcon} from 'lucide-react';
import {useCallback, useState, useMemo, useEffect, useRef, lazy, Suspense} from 'react';
import {useFileProcessing} from './hooks/useFileProcessing';
import {useTestHistory} from './hooks/useTestHistory';
import {Alert, AlertTitle, AlertDescription} from './components/ui/Alert';
import {Button} from './components/ui/Button';
import {FileUploadZone} from './components/FileUploadZone';
import {ResultsSummary} from './components/ResultsSummary';
import {LoadingSpinner} from './components/LoadingSpinner';

// Lazy load heavy components for better code splitting (React 19 best practice)
const TestResultItem = lazy(() => import('./components/TestResultItem').then(m => ({ default: m.TestResultItem })));
const FilterControls = lazy(() => import('./components/FilterControls').then(m => ({ default: m.FilterControls })));
const HistoryPanel = lazy(() => import('./components/HistoryPanel').then(m => ({ default: m.HistoryPanel })));

const TestResultsViewer = () => {
    const [showHistory, setShowHistory] = useState(false);
    const [currentHistoryId, setCurrentHistoryId] = useState<string | null>(null);
    const lastSavedResultsRef = useRef<string | null>(null);

    // File processing hook
    const {
        results: currentResults,
        isPending,
        error,
        uploadMode,
        expandedResult,
        resultCounts,
        setUploadMode,
        handleFileDrop,
        resetResults
    } = useFileProcessing();

    // History management hook
    const {
        history,
        currentFilter,
        setCurrentFilter,
        filteredResults,
        addToHistory,
        deleteEntry,
        clearAllHistory,
    } = useTestHistory(currentResults);

    // Automatically save to history when results change (React 19 best practice: useEffect)
    useEffect(() => {
        if (currentResults && !isPending) {
            // Create a unique identifier for the results to prevent duplicate saves
            const resultsId = JSON.stringify(currentResults.processedAt);

            if (resultsId !== lastSavedResultsRef.current) {
                addToHistory(currentResults, uploadMode);
                lastSavedResultsRef.current = resultsId;
            }
        }
    }, [currentResults, uploadMode, isPending, addToHistory]);

    // Load history entry and display it (React 19 best practice: useCallback)
    const handleLoadHistory = useCallback((id: string) => {
        setCurrentHistoryId(id);
    }, []);

    // Get the active results to display (current or from history)
    // Uses history array instead of localStorage for better performance
    const displayResults = useMemo(() => {
        if (currentHistoryId) {
            const entry = history.find(h => h.id === currentHistoryId);
            return entry?.report || null;
        }
        return filteredResults;
    }, [currentHistoryId, filteredResults, history]);

    // Handle reset with history clearing
    const handleReset = useCallback(() => {
        resetResults();
        setCurrentHistoryId(null);
    }, [resetResults]);

    // Handle deletion of history entry with cleanup
    const handleDeleteEntry = useCallback((id: string) => {
        // Clear currentHistoryId if we're deleting the currently viewed entry
        if (currentHistoryId === id) {
            setCurrentHistoryId(null);
        }
        deleteEntry(id);
    }, [currentHistoryId, deleteEntry]);

    // Toggle history panel
    const toggleHistory = useCallback(() => {
        setShowHistory(prev => !prev);
    }, []);

    return (
        <div className="min-h-screen bg-gradient-to-br from-slate-50 via-blue-50 to-slate-50">
            <div className="container mx-auto px-6 py-12 max-w-7xl">
                {/* Header */}
                <div className="flex justify-between items-center mb-12 backdrop-blur-sm bg-white/30 rounded-2xl p-6 shadow-lg">
                    <button
                        className="text-4xl font-bold bg-gradient-to-r from-blue-700 to-indigo-700 bg-clip-text text-transparent hover:from-blue-800 hover:to-indigo-800 transition-all duration-300 transform hover:scale-102"
                        onClick={handleReset}
                    >
                        Test Results Viewer
                    </button>
                    <div className="flex gap-3">
                        {history.length > 0 && (
                            <Button
                                variant="outline"
                                onClick={toggleHistory}
                                className="hover:shadow-lg transition-all duration-300 border-2 border-purple-200 hover:border-purple-300 flex items-center gap-2"
                            >
                                <HistoryIcon className="w-4 h-4" />
                                {showHistory ? 'Hide' : 'Show'} History
                            </Button>
                        )}
                        {displayResults && (
                            <Button
                                variant="outline"
                                onClick={handleReset}
                                className="hover:shadow-lg transition-all duration-300 border-2 border-blue-200 hover:border-blue-300"
                            >
                                Upload New Files
                            </Button>
                        )}
                    </div>
                </div>

                <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
                    {/* Main Content */}
                    <div className={`space-y-8 ${showHistory ? 'lg:col-span-2' : 'lg:col-span-3'}`}>
                        {error && (
                            <Alert
                                variant="destructive"
                                className="mb-6 shadow-lg animate-fade-in"
                                icon={<AlertCircle className="h-5 w-5"/>}
                            >
                                <AlertTitle>Error Processing Files</AlertTitle>
                                <AlertDescription>{error}</AlertDescription>
                            </Alert>
                        )}

                        {!displayResults && !isPending && (
                            <div className="transform transition-all duration-300 hover:scale-[1.02]">
                                <FileUploadZone
                                    uploadMode={uploadMode}
                                    setUploadMode={setUploadMode}
                                    handleFileDrop={handleFileDrop}
                                />
                            </div>
                        )}

                        {isPending && (
                            <div className="flex justify-center items-center py-20">
                                <LoadingSpinner />
                            </div>
                        )}

                        {displayResults && (
                            <div className="space-y-8 animate-fade-in">
                                <div className="backdrop-blur-sm bg-white/40 rounded-2xl p-6 shadow-lg">
                                    <ResultsSummary results={displayResults}/>
                                </div>

                                {/* Filter Controls */}
                                {displayResults.comparison.length > 0 && (
                                    <Suspense fallback={<div className="h-20 flex items-center justify-center"><LoadingSpinner /></div>}>
                                        <FilterControls
                                            currentFilter={currentFilter}
                                            onFilterChange={setCurrentFilter}
                                            totalCount={resultCounts.total}
                                            passedCount={resultCounts.passed}
                                            failedCount={resultCounts.failed}
                                        />
                                    </Suspense>
                                )}

                                {/* Test Results */}
                                {displayResults.comparison.length > 0 && (
                                    <Suspense fallback={<div className="flex justify-center py-10"><LoadingSpinner /></div>}>
                                        <div className="space-y-6">
                                            {displayResults.comparison.map((result, index) => (
                                                <div key={`${result.callId}-${index}`}
                                                     className="transform transition-all duration-300 hover:scale-[1.01]">
                                                    <TestResultItem
                                                        result={result}
                                                        index={index}
                                                        expandedResult={expandedResult}
                                                    />
                                                </div>
                                            ))}
                                        </div>
                                    </Suspense>
                                )}

                                {displayResults.comparison.length === 0 && (
                                    <Alert
                                        variant="default"
                                        className="shadow-lg"
                                    >
                                        <AlertTitle>No Results Found</AlertTitle>
                                        <AlertDescription>
                                            No {currentFilter === 'all' ? '' : `${currentFilter} `}tests found with the current filter.
                                        </AlertDescription>
                                    </Alert>
                                )}
                            </div>
                        )}
                    </div>

                    {/* History Sidebar */}
                    {showHistory && (
                        <div className="lg:col-span-1">
                            <div className="sticky top-6">
                                <Suspense fallback={<div className="h-40 flex items-center justify-center"><LoadingSpinner /></div>}>
                                    <HistoryPanel
                                        history={history}
                                        onLoadHistory={handleLoadHistory}
                                        onDeleteEntry={handleDeleteEntry}
                                        onClearAll={clearAllHistory}
                                        currentHistoryId={currentHistoryId}
                                    />
                                </Suspense>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};

export default TestResultsViewer;
