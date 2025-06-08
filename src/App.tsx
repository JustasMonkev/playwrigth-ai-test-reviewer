import {AlertCircle} from 'lucide-react';
import {useFileProcessing} from './hooks/useFileProcessing';
import {Alert, AlertTitle, AlertDescription} from './components/ui/Alert';
import {Button} from './components/ui/Button';
import {FileUploadZone} from './components/FileUploadZone';
import {TestResultItem} from './components/TestResultItem';
import {ResultsSummary} from './components/ResultsSummary';
import {LoadingSpinner} from './components/LoadingSpinner';

const TestResultsViewer = () => {
    const {
        results,
        isPending,
        error,
        uploadMode,
        expandedResult,
        setUploadMode,
        handleFileDrop,
        resetResults
    } = useFileProcessing();

    return (
        <div className="min-h-screen bg-gradient-to-br from-slate-50 via-blue-50 to-slate-50">
            <div className="container mx-auto px-6 py-12 max-w-7xl">
                <div className="flex justify-between items-center mb-12 backdrop-blur-sm bg-white/30 rounded-2xl p-6 shadow-lg">
                    <button 
                        className="text-4xl font-bold bg-gradient-to-r from-blue-700 to-indigo-700 bg-clip-text text-transparent hover:from-blue-800 hover:to-indigo-800 transition-all duration-300 transform hover:scale-102"
                        onClick={resetResults}
                    >
                        Test Results Viewer
                    </button>
                    {results && (
                        <Button 
                            variant="outline" 
                            onClick={resetResults} 
                            className="hover:shadow-lg transition-all duration-300 border-2 border-blue-200 hover:border-blue-300"
                        >
                            Upload New Files
                        </Button>
                    )}
                </div>

                <div className="space-y-8">
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

                    {!results && !isPending && (
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

                    {results && (
                        <div className="space-y-8 animate-fade-in">
                            <div className="backdrop-blur-sm bg-white/40 rounded-2xl p-6 shadow-lg">
                                <ResultsSummary results={results}/>
                            </div>

                            {results.comparison.length > 0 && (
                                <div className="space-y-6">
                                    {results.comparison.map((result, index) => (
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
                            )}
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};

export default TestResultsViewer;
