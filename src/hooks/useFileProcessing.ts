import {useState, useCallback, useTransition, useMemo} from 'react';
import {analyzeTrace, compareTraces, CombinedReport} from '../../utils';

export const useFileProcessing = () => {
    const [results, setResults] = useState<CombinedReport | null>(null);
    const [isPending, startTransition] = useTransition();
    const [error, setError] = useState<string | null>(null);
    const [uploadMode, setUploadMode] = useState<'single' | 'multiple'>('single');
    const [expandedResult, setExpandedResult] = useState<string | null>(null);

    // Memoize result counts for performance (React 19 best practice)
    const resultCounts = useMemo(() => {
        if (!results) return { total: 0, passed: 0, failed: 0 };

        let passed = 0;
        let failed = 0;

        results.comparison.forEach(detail => {
            if (detail.details.failure) {
                failed++;
            } else if (detail.details.passedTest) {
                passed++;
            }
        });

        return { total: results.comparison.length, passed, failed };
    }, [results]);

    const handleFileDrop = useCallback(async (e: React.DragEvent<HTMLDivElement> | React.ChangeEvent<HTMLInputElement>) => {
        e.preventDefault();
        const files = "dataTransfer" in e ? e.dataTransfer?.files : e.target.files;

        if (!files?.length) return;

        startTransition(async () => {
            try {
                const fileList = Array.from(files);

                // Validate file count based on upload mode
                if (uploadMode === 'single' && fileList.length > 1) {
                    setError('Cannot analyze multiple files when Single File Analysis is selected. Please select only one ZIP file or switch to Two-File Comparison mode.');
                    return;
                }

                if (uploadMode === 'multiple' && fileList.length !== 2) {
                    setError('Two-File Comparison mode requires exactly two ZIP files. Please upload two files or switch to Single File Analysis mode.');
                    return;
                }

                if (uploadMode === 'single' && fileList.length === 1) {
                    setResults(await analyzeTrace(fileList[0]));
                } else if (uploadMode === 'multiple' && fileList.length === 2) {
                    setResults(await compareTraces(fileList));
                }

                setError(null);
            } catch (err) {
                setError(err instanceof Error ? err.message : 'An unexpected error occurred while processing the files.');
                setResults(null);
            }
        });
    }, [uploadMode]);

    const resetResults = useCallback(() => {
        setResults(null);
        setError(null);
        setExpandedResult(null);
    }, []);

    return {
        results,
        isPending,
        error,
        uploadMode,
        expandedResult,
        resultCounts,
        setUploadMode,
        setExpandedResult,
        handleFileDrop,
        resetResults
    };
};
