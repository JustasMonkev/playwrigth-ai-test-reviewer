import {CombinedReport} from '../types';
import {Alert, AlertTitle, AlertDescription} from './ui/Alert';

interface ResultsSummaryProps {
    results: CombinedReport;
}

export const ResultsSummary = ({results}: ResultsSummaryProps) => {
    const {comparison, processedAt} = results;
    const hasResults = comparison.length > 0;

    return (
        <Alert
            variant={hasResults ? "destructive" : "success"}
            className="mb-4"
        >
            <AlertTitle>
                {hasResults ? "Analysis Complete" : "No Test Failures Found"}
            </AlertTitle>
            <AlertDescription>
                {hasResults ? (
                    <>
                        Processed on {new Date(processedAt).toLocaleString()} •{' '}
                        {comparison.length} result{comparison.length !== 1 ? 's' : ''} found
                    </>
                ) : (
                    "The analysis completed successfully, but no test failures were detected"
                )}
            </AlertDescription>
        </Alert>
    );
};
