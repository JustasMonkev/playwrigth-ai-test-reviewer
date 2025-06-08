export interface SoftAssertion {
    selector: string;
    expectedText: string;
    actualText?: string;
    message: string;
    apiName: string;
    isNot?: boolean;
    timeout?: number;
    passed: boolean;
}

export interface ProcessedRawData {
    testId: string;
    callId: string;
    htmlSnapshot?: any[];
    errorMessage?: string;
    errorLogs?: string[];
    failedScreenshot?: string | null;
    passedScreenshot?: string | null;
    testParams?: any;
    softAssertions?: SoftAssertion[];
    info?: {
        error?: {
            name?: string;
            message?: string;
        };
        result?: {
            log?: string[];
            [key: string]: any;
        };
    };
}

export interface TestResult {
    testId: string;           
    callId: string;           
    type?: string;            
    endTime?: number;
    error?: any;              
    result?: any;             
    failedScreenshot?: string | null; 
    passedScreenshot?: string | null; 
    processedRawData?: ProcessedRawData; 
    params?: any;  
    other?: [k: string];
    softAssertions?: SoftAssertion[];
}

export interface ZipReport {
    zipPath: string;
    errors: TestResult[];
}

export interface ComparisonDetail {
    callId: string;
    details: {
        failure?: TestResult;
        passedTest?: TestResult;
    };
    other?: TestResult[];
}

export interface CombinedReport {
    processedAt: string;
    comparison: ComparisonDetail[];
}