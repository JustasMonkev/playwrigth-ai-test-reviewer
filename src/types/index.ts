export interface ProcessedRawData {
    pageId?: string;
    snapshot?: any;
    info?: any;
    allData?: any[];
    testParams?: any;
    errorMessage?: string;
    [key: string]: any;
}

export interface TestResult {
    testId: string;
    callId: string;
    type?: string;
    endTime?: number;
    error?: any;
    result?: any;    failedScreenshot?: string | null;
    passedScreenshot?: string | null;
    processedRawData?: ProcessedRawData;
    params?: any;
}

export interface ComparisonDetail {
    callId: string;
    details: {
        failure?: TestResult;
        passedTest?: TestResult;
    };
}

export interface CombinedReport {
    processedAt: string;
    comparison: ComparisonDetail[];
}