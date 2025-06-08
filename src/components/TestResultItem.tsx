import {useState} from 'react';
import {AlertCircle, Code, BarChart, AlertTriangle} from 'lucide-react';
import {ComparisonDetail} from '../types';
import {Card, CardHeader, CardContent} from './ui/Card';
import {Alert, AlertTitle, AlertDescription} from './ui/Alert';
import {Badge} from './ui/Badge';
import {Button} from './ui/Button';
import {apiKey, systemPrompt, userMessage} from '../../utils/apiCall';
import CopyButton from "./ui/CopyButton.tsx";
import {smoothStream, streamText} from "ai";
import {createGoogleGenerativeAI} from "@ai-sdk/google";
import Markdown from "react-markdown";

interface TestResultItemProps {
    result: ComparisonDetail;
    index: number;
    expandedResult: string | null;
}

export const TestResultItem = ({result}: TestResultItemProps) => {
    const {details} = result;
    const hasFailure = Boolean(details?.failure);
    const [showRawData, setShowRawData] = useState(false);
    const [isAnalyzing, setIsAnalyzing] = useState(false);
    const [isDisabled, setIsDisabled] = useState(false);
    const [streamedText, setStreamedText] = useState('');
    const testResult = hasFailure ? details.failure : details.passedTest;
    const processedRawData = testResult?.processedRawData;

    // If no processed raw data is available, show a message
    if (!processedRawData) {
        return (
            <Card className="mb-6 overflow-hidden">
                <CardHeader>
                    <div className="flex items-center space-x-3">
                        <h3 className="font-medium">Result</h3>
                        <Badge variant="error">NO DATA</Badge>
                    </div>
                </CardHeader>
                <CardContent>
                    <p className="text-gray-500">No processed data available for this test result.</p>
                </CardContent>
            </Card>
        );
    }

    // Create simplified data structure for AI analysis
    const rawData = {
        testId: processedRawData.testId,
        callId: processedRawData.callId,
        htmlSnapshot: processedRawData.htmlSnapshot || [],
        errorMessage: processedRawData.errorMessage,
        errorLogs: processedRawData.errorLogs || [],
        softAssertions: processedRawData.softAssertions || [],
        info: processedRawData.info || {}
    };

    // Extract test parameters and error information
    const testParams = processedRawData.testParams;
    const errorMessage = processedRawData.errorMessage || "No detailed error message available";
    const errorLogs = processedRawData.errorLogs || [];

    // Get screenshots from both failed and passed test results
    const failedScreenshot = details.failure?.failedScreenshot || details.failure?.processedRawData?.failedScreenshot;
    const passedScreenshot = details.passedTest?.passedScreenshot || details.passedTest?.processedRawData?.passedScreenshot;

    // Function to analyze the test with AI
    const handleAnalyzeClick = async () => {
        if (isAnalyzing) return;
        setIsDisabled(true);
        setIsAnalyzing(true);
        setStreamedText(''); // Reset streamed text

        try {
            const customGoogle = createGoogleGenerativeAI({
                apiKey
            });

            //
            const model = customGoogle('gemini-2.5-pro-preview-05-06');
            const result = streamText({
                model: model,
                system: systemPrompt,
                prompt: userMessage(errorMessage, rawData, testParams),
                experimental_transform: smoothStream({
                    chunking: 'line',
                }),
            });

            let accumulatedText = "";
            for await (const textPart of result.textStream) {
                accumulatedText += textPart.replace('```markdown', '').replace('```', '');
                setStreamedText(accumulatedText);
            }
        } catch (error) {
            console.error("Error analyzing test:", error);
        } finally {
            setIsAnalyzing(false);
        }
    };

    const handleCopyRawData = async () => {
        await navigator.clipboard.writeText('');
        await navigator.clipboard.writeText(JSON.stringify(rawData, null, 2));
    }

    return (
        <Card className="mb-6 overflow-hidden">
            <CardHeader className="flex justify-between items-center">
                <div>
                    <div className="flex items-center space-x-3">
                        <h3 className="font-medium">Result</h3>
                        <Badge variant={hasFailure ? 'error' : 'success'}>
                            {hasFailure ? 'FAILED' : 'PASSED'}
                        </Badge>
                    </div>
                </div>
            </CardHeader> {/* Screenshot comparison */}
            <div className="p-4 bg-gray-50 border-t border-b border-gray-100">
                {/* Dynamically adjust layout based on available screenshots */}
                {passedScreenshot && failedScreenshot ? (
                    // Both screenshots available - show side by side
                    <div className="grid grid-cols-2 gap-6">
                        {/* Passed screenshot */}
                        <div>
                            <h4 className="text-sm font-medium text-gray-900 mb-2">Passed Screenshot</h4>
                            <div
                                className={`border rounded-lg overflow-hidden bg-white ${!hasFailure ? 'ring-2 ring-green-500' : ''}`}>
                                <img
                                    src={passedScreenshot}
                                    alt="Passed test screenshot"
                                    className="w-full h-auto object-cover"
                                />
                            </div>
                        </div>
                        {/* Failed screenshot */}
                        <div>
                            <h4 className="text-sm font-medium text-gray-900 mb-2">Failed Screenshot</h4>
                            <div
                                className={`border rounded-lg overflow-hidden bg-white ${hasFailure ? 'ring-2 ring-red-500' : ''}`}>
                                <img
                                    src={failedScreenshot}
                                    alt="Failed test screenshot"
                                    className="w-full h-auto object-cover"
                                />
                            </div>
                        </div>
                    </div>
                ) : !passedScreenshot && failedScreenshot ? (
                    // Only failed screenshot available - show it full width
                    <div>
                        <h4 className="text-sm font-medium text-gray-900 mb-2">Failed Screenshot</h4>
                        <div
                            className={`border rounded-lg overflow-hidden bg-white ${hasFailure ? 'ring-2 ring-red-500' : ''}`}>
                            <img
                                src={failedScreenshot}
                                alt="Failed test screenshot"
                                className="w-full h-auto object-cover"
                            />
                        </div>
                    </div>
                ) : passedScreenshot && !failedScreenshot ? (
                    // Only passed screenshot available
                    <div>
                        <h4 className="text-sm font-medium text-gray-900 mb-2">Passed Screenshot</h4>
                        <div
                            className={`border rounded-lg overflow-hidden bg-white ${!hasFailure ? 'ring-2 ring-green-500' : ''}`}>
                            <img
                                src={passedScreenshot}
                                alt="Passed test screenshot"
                                className="w-full h-auto object-cover"
                            />
                        </div>
                    </div>
                ) : (
                    // No screenshots available
                    <div className="bg-gray-100 w-full py-12 flex items-center justify-center rounded-lg">
                        <span className="text-gray-500">No screenshots found</span>
                    </div>
                )}
            </div>

            <CardContent>
                {hasFailure && details.failure && (
                    <div className="mb-4">
                        {/* Soft Assertions Summary */}
                        {processedRawData.softAssertions && processedRawData.softAssertions.length > 0 && (
                            <div className="mb-4">
                                <Alert
                                    variant="warning"
                                    className="mb-4"
                                    icon={<AlertTriangle className="h-5 w-5"/>}
                                >
                                    <AlertTitle>Soft Assertions Failed</AlertTitle>
                                    <AlertDescription>
                                        <div className="space-y-2 mt-2">
                                            {processedRawData.softAssertions
                                                .filter((sa: { passed: boolean }) => !sa.passed)
                                                .map((sa: {
                                                    selector: string,
                                                    expectedText: string,
                                                    actualText?: string
                                                }, idx: number) => (
                                                    <div key={idx} className="border-l-2 border-orange-400 pl-3 py-1">
                                                        <div className="text-sm font-medium">
                                                            Selector: <code
                                                            className="bg-gray-100 px-1 rounded">{sa.selector}</code>
                                                        </div>
                                                        <div className="text-sm text-gray-600">
                                                            Expected: <span
                                                            className="font-mono text-green-600">"{sa.expectedText}"</span>
                                                        </div>
                                                        {sa.actualText && (
                                                            <div className="text-sm text-gray-600">
                                                                Actual: <span
                                                                className="font-mono text-red-600">"{sa.actualText}"</span>
                                                            </div>
                                                        )}
                                                    </div>
                                                ))}
                                        </div>
                                    </AlertDescription>
                                </Alert>
                            </div>
                        )}

                        <Alert
                            variant="destructive"
                            className="mb-4"
                            icon={<AlertCircle className="h-5 w-5"/>}
                        >
                            <AlertTitle>Failed Reason:</AlertTitle>
                            <AlertDescription>
                                {errorLogs?.length > 0 ? errorLogs.join("\n") : errorMessage}
                            </AlertDescription>
                        </Alert>

                        {/* AI Analysis button for failure cases */}
                        <div className="mt-4 flex justify-between items-center">
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={handleAnalyzeClick}
                                disabled={isDisabled}
                                className={`flex items-center transition-colors duration-200 ${isDisabled ? 'disabled:border-gray-200 disabled:bg-gray-50 disabled:text-gray-500 disabled:shadow-none dark:disabled:border-gray-700 dark:disabled:bg-gray-800/20' : ''}`}
                            >
                                {isAnalyzing ? (
                                    <>
                                        <div
                                            className="animate-spin w-4 h-4 mr-2 border-2 border-gray-400 border-t-primary rounded-full"/>
                                        Analyzing...
                                    </>
                                ) : (
                                    <>
                                        <BarChart className="w-4 h-4 mr-2"/>
                                        Analyze with AI
                                    </>
                                )}
                            </Button>

                            {/* Toggle for showing raw data */}
                            {rawData && (
                                <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={() => setShowRawData(!showRawData)}
                                    className="flex items-center transition-colors duration-200"
                                >
                                    <Code className="w-4 h-4 mr-2"/>
                                    {showRawData ? 'Hide Raw Data' : 'Show Raw Data'}
                                </Button>
                            )}
                        </div>

                        {/* AI Analysis Results with improved markdown rendering */}
                        {streamedText && (
                            <div className="mt-4 border rounded-lg p-4 bg-gray-50">
                                <div className="flex justify-between items-start mb-2">
                                    <h4 className="font-medium text-gray-900">AI Analysis</h4>
                                </div>
                                <div className="mt-2 space-y-3">
                                    <div>
                                        <h5 className="text-sm font-medium text-gray-700 mb-2">Cause of Failure:</h5>
                                        <div
                                            className="prose prose-sm max-w-none whitespace-pre-line [&>*]:!my-1 [&>p]:!my-1 [&>ul]:!mt-1 [&>ul]:!mb-1">
                                            <Markdown>{streamedText}</Markdown>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        )}
                        {/* Raw data container */}
                        <div
                            className={`transition-all duration-300 ease-in-out ${
                                showRawData ? 'max-h-[500px] opacity-100 mt-2' : 'max-h-0 opacity-0 overflow-hidden'
                            }`}
                        >
                            <div className="relative">
                                <CopyButton handleCopyRawData={handleCopyRawData}/>
                                <pre
                                    className="bg-gray-100 p-4 rounded-lg text-sm overflow-auto max-h-[450px] text-gray-800">
                                    {JSON.stringify(details, null, 2)}
                                </pre>
                            </div>
                        </div>
                    </div>
                )}
            </CardContent>
        </Card>
    );
};
