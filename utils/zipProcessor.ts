import JSZip from 'jszip';
import { findMatchingResult, findMatchingResultsByCallId, removeElements } from "./utils";
import { TestResult, ZipReport, ComparisonDetail, CombinedReport, ProcessedRawData, SoftAssertion } from './types';

const screenshotBlobs = new Map<string, string>();


function extractSoftAssertions(traceData: any[]): SoftAssertion[] {
    const softAssertions: SoftAssertion[] = [];
    const softAssertionCalls = new Map<string, any>();


    if (!traceData || traceData.length === 0) {
        return softAssertions;
    }


    for (let i = 0; i < traceData.length; i++) {
        const entry = traceData[i];
        if (entry?.type === 'before' && entry?.apiName?.includes('expect.soft')) {
            softAssertionCalls.set(entry.callId, entry);
        }
    }


    if (softAssertionCalls.size === 0) {
        return softAssertions;
    }


    for (let i = 0; i < traceData.length; i++) {
        const entry = traceData[i];
        if (entry?.type === 'after' && entry?.callId && softAssertionCalls.has(entry.callId)) {
            const callData = softAssertionCalls.get(entry.callId);
            const params = callData.params || {};

            const softAssertion: SoftAssertion = {
                selector: params.selector || '',
                expectedText: params.expectedText?.[0]?.string || '',
                message: entry.error?.message || callData.apiName || '',
                apiName: callData.apiName || '',
                isNot: params.isNot || false,
                timeout: params.timeout || 5000,
                passed: !entry.error
            };


            if (entry.result?.received?.s) {
                softAssertion.actualText = entry.result.received.s;
            } else if (entry.error?.message) {

                const match = entry.error.message.match(/unexpected value "(.+?)"/);
                if (match) {
                    softAssertion.actualText = match[1];
                }
            }

            softAssertions.push(softAssertion);
        }
    }

    return softAssertions;
}

function findMostRelevantError(errors: TestResult[]): TestResult | undefined {
    if (errors.length === 0) return undefined;


    const errorsWithMessages = errors.filter(error =>
        error.error &&
        error.error.message &&
        error.error.message.trim() !== ''
    );
    if (errorsWithMessages.length > 0) {
        return errorsWithMessages.sort((a, b) => {
            const timeA = a.endTime || 0;
            const timeB = b.endTime || 0;
            return timeB - timeA;
        })[0];
    }

    return errors.sort((a, b) => {
        const timeA = a.endTime || 0;
        const timeB = b.endTime || 0;
        return timeB - timeA;
    })[0];
}

function groupTestResultsByTest(results: TestResult[]): Map<string, TestResult[]> {
    const testMap = new Map<string, TestResult[]>();

    results.forEach(result => {
        if (!result || !result.callId) {
            console.warn('Test result without callId found, skipping:', result);
            return;
        }

        const match = result.callId.match(/^([^@]+)/);
        const testIdentifier = match ? match[1] : result.callId;

        if (!testMap.has(testIdentifier)) {
            testMap.set(testIdentifier, []);
        }
        testMap.get(testIdentifier)!.push(result);
    });

    return testMap;
}

async function processTraceFile(content: string, shouldReturnPassedTest = false): Promise<TestResult[]> {
    if (!content) {
        return [{
            testId: 'invalid-content',
            callId: 'invalid-content@1',
            type: 'error',
            error: {
                message: 'Invalid or empty trace file content'
            },
            result: {
                log: ['The trace file content is invalid or empty.']
            }
        }];
    }

    try {
        const lines = content.split('\n');
        const results: any[] = [];


        const BATCH_SIZE = 1000;
        const totalLines = lines.length;

        for (let i = 0; i < totalLines; i += BATCH_SIZE) {
            const batch = lines.slice(i, Math.min(i + BATCH_SIZE, totalLines));

            for (const line of batch) {
                const trimmedLine = line.trim();
                if (!trimmedLine) continue;

                try {
                    results.push(JSON.parse(trimmedLine));
                } catch (e) {


                    try {
                        const fixedLine = trimmedLine.replace(/}\{/g, '},{');
                        const wrappedJson = `[${fixedLine}]`;
                        const parsed = JSON.parse(wrappedJson);
                        results.push(...parsed);
                    } catch (nestedError) {

                        if (results.length < 10) {
                            console.warn(`Error parsing JSON line even with fixes: ${nestedError}`, e);
                        }
                    }
                }
            }
        }


        if (results.length === 0 && content.includes('}{')) {
            try {
                const jsonStrings = content.split(/(?<=})(?={)/g);
                for (const str of jsonStrings) {
                    try {
                        results.push(JSON.parse(str.trim()));
                    } catch (err) {
                        console.warn('Error parsing split JSON string:', err);
                    }
                }
            } catch (splitError) {
                console.warn('Error splitting concatenated JSON:', splitError);
            }
        }


        const softAssertions = extractSoftAssertions(results);


        const data = findMatchingResult(results);


        if (shouldReturnPassedTest) {
            return [data]
        }


        if (!data || !data.callId) {
            return []
        }

        const testResults = findMatchingResultsByCallId(data.callId, results);


        if (softAssertions.length > 0) {
            testResults.forEach(result => {
                result.softAssertions = softAssertions;
            });
        }

        return testResults;
    } catch (e) {
        console.error('Error in fallback parsing:', e);
        return [];
    }
}


async function processJsonFile(jsonFile: JSZip.JSZipObject): Promise<TestResult[]> {
    const jsonContent = await jsonFile.async('text');
    const jsonData = JSON.parse(jsonContent);


    const testResult: TestResult = {
        testId: 'json-test',
        callId: 'json-call@1',
        type: 'error',
        error: {
            message: 'Playwright assertion failed: Element not found'
        },
        result: {
            log: ['Test data extracted from JSON file', jsonFile.name]
        },
    };


    if (Array.isArray(jsonData) && jsonData.length > 0) {
        const firstEntry = jsonData[0];
        if (firstEntry.type === 'before' && firstEntry.apiName) {
            testResult.testId = firstEntry.stepId || firstEntry.callId || 'json-test';
            testResult.callId = firstEntry.callId || 'json-call@1';
            testResult.error.message = `Playwright assertion failed: ${firstEntry.apiName} - Expected element matching "${firstEntry.params?.selector || 'unknown selector'}" to be visible`;
            testResult.result.log = [
                `Selector: ${firstEntry.params?.selector || 'unknown'}`,
                `Method: ${firstEntry.method || 'unknown'}`,
                `API: ${firstEntry.apiName || 'unknown'}`,
                `Expected: ${firstEntry.params?.expression || 'unknown'}`
            ];
        }
    }

    return [testResult];
}


async function processScreenshots(screenshotFiles: JSZip.JSZipObject[]): Promise<void> {
    for (const screenshotFile of screenshotFiles) {
        const blob = await screenshotFile.async('blob');
        const blobUrl = URL.createObjectURL(blob);


        const fullPath = screenshotFile.name;
        const filename = fullPath.split('/').pop() || '';
        const nameWithoutExt = filename.replace(/\.(png|jpg|jpeg|gif|bmp|webp|svg)$/i, '');


        screenshotBlobs.set(filename, blobUrl);
        screenshotBlobs.set(nameWithoutExt, blobUrl);
        screenshotBlobs.set(fullPath, blobUrl);


        const pageIdMatch = nameWithoutExt.match(/page@[a-f0-9]+/);
        if (pageIdMatch) {
            screenshotBlobs.set(pageIdMatch[0], blobUrl);
        }
    }
}


function filterRelevantErrors(allErrors: TestResult[]): TestResult[] {
    if (allErrors.length === 0) {
        return [];
    }

    const groupedErrors = groupTestResultsByTest(allErrors);
    const filteredErrors: TestResult[] = [];

    groupedErrors.forEach((errors) => {
        const errorsWithError = errors.filter(e => e.error);

        const findBestScreenshot = (testResult: any): string | undefined => {
            // If testResult has pageId, try to find screenshot
            if (testResult.pageId) {
                const pageScreenshot = screenshotBlobs.get(testResult.pageId);
                if (pageScreenshot) return pageScreenshot;
            }

            // Check _allData for pageId
            if (testResult._allData && Array.isArray(testResult._allData)) {
                for (const data of testResult._allData) {
                    if (data.pageId) {
                        const pageScreenshot = screenshotBlobs.get(data.pageId);
                        if (pageScreenshot) return pageScreenshot;
                    }
                }
            }

            // Try to find matching screenshot by pageId substring
            const screenshotKeys = Array.from(screenshotBlobs.keys());
            if (testResult.pageId) {
                const matchingKey = screenshotKeys.find(key => key.includes(testResult.pageId));
                if (matchingKey) return screenshotBlobs.get(matchingKey);
            }

            // Fallback to last screenshot
            if (screenshotKeys.length > 0) {
                return screenshotBlobs.get(screenshotKeys[screenshotKeys.length - 1]);
            }

            return undefined;
        };

        if (errorsWithError.length > 0) {
            // Handle failed tests
            const relevantError = findMostRelevantError(errorsWithError);
            if (relevantError) {
                if (!relevantError.type) {
                    relevantError.type = 'error';
                }

                const screenshotUrl = findBestScreenshot(relevantError);
                if (screenshotUrl) {
                    relevantError.failedScreenshot = screenshotUrl;
                }

                const processedData = processRawDataForResult(relevantError);
                if (processedData) {
                    processedData.failedScreenshot = screenshotUrl;
                    relevantError.processedRawData = processedData;
                }

                filteredErrors.push(relevantError);
            }
        } else {
            const representativeResult = errors[0];
            if (representativeResult) {
                const screenshotUrl = findBestScreenshot(representativeResult);
                if (screenshotUrl) {
                    representativeResult.passedScreenshot = screenshotUrl;
                }
                // Ensure type is set if not present (e.g., to 'passed')
                if (!representativeResult.type) {
                    // representativeResult.type = 'passed'; // Or handle as appropriate
                }
                filteredErrors.push(representativeResult);
            }
        }
    });

    return filteredErrors.length > 0 ? filteredErrors : allErrors;
}

function processRawDataForResult(testResult: any): ProcessedRawData | undefined {
    if (!testResult) {
        return undefined;
    }

    const callId = testResult.callId || 'unknown';
    const processedData: ProcessedRawData = {
        testId: testResult.testId || callId,
        callId: callId,
        errorMessage: testResult.error?.message || "No error message available",
        errorLogs: testResult.result?.log || [],
        testParams: testResult.params
    };


    if (testResult.softAssertions && testResult.softAssertions.length > 0) {
        processedData.softAssertions = testResult.softAssertions;

        const failedSoftAssertions = testResult.softAssertions.filter((sa: SoftAssertion) => !sa.passed);
        if (failedSoftAssertions.length > 0) {
            processedData.errorMessage = `${failedSoftAssertions.length} soft assertion(s) failed. ${processedData.errorMessage}`;
        }
    }

    if (testResult._allData && Array.isArray(testResult._allData)) {
        let largestHtml = null;
        let largestSize = 0;
        for (let i = 0; i < testResult._allData.length; i++) {
            const element = testResult._allData[i];

            if (element.type === 'frame-snapshot' && element.snapshot?.html) {
                const htmlSize = JSON.stringify(element.snapshot.html).length;

                if (htmlSize > largestSize) {
                    largestSize = htmlSize;
                    largestHtml = element;
                }
            }
        }
        if (largestHtml) {
            const htmlCopy = JSON.parse(JSON.stringify(largestHtml.snapshot.html));
            removeElements(htmlCopy, ["HEAD", "STYLE", "META"]);
            processedData.htmlSnapshot = htmlCopy;
        }
    }

    return processedData;
}

export async function processZipFile(file: File, shouldReturnPassedTest = false): Promise<ZipReport> {
    const zip = new JSZip();
    const contents = await zip.loadAsync(file);
    let allErrors: TestResult[] = [];


    const traceFiles = contents.file(/0-trace\.trace$/);

    if (traceFiles && traceFiles.length > 0) {

        const traceFile = traceFiles[0];
        const traceContent = await traceFile.async('text');
        allErrors = await processTraceFile(traceContent, shouldReturnPassedTest);
    } else {

        const jsonFiles = contents.file(/\.json$/);

        if (!jsonFiles || jsonFiles.length === 0) {
            throw new Error('No trace or JSON files found in zip');
        }


        allErrors = await processJsonFile(jsonFiles[0]);
    }


    const screenshotFiles = contents.file(/\.(png|jpg|jpeg|gif|bmp|webp|svg)$/i);

    if (screenshotFiles && screenshotFiles.length > 0) {
        await processScreenshots(screenshotFiles);
    } if (shouldReturnPassedTest) {
        // For passed tests, ensure we have proper TestResult objects
        allErrors = allErrors.map(error => {
            // If it's a frame-snapshot or other raw data, convert it
            if (!error.testId || !error.callId) {
                const errorAny = error as any;
                return {
                    testId: errorAny.snapshot?.callId || 'passed-test',
                    callId: errorAny.snapshot?.callId || 'passed-test@1',
                    type: 'passed',
                    result: error,
                    other: errorAny.snapshot?.pageId || 'unknown'
                } as TestResult;
            }
            return error;
        });

        // Assign screenshots to passed tests
        allErrors = allErrors.map(testResult => {
            const findBestScreenshot = (testResult: any): string | undefined => {
                if (testResult.other) {
                    const pageScreenshot = screenshotBlobs.get(testResult.other);
                    if (pageScreenshot) return pageScreenshot;
                }

                // Check result.snapshot for pageId
                if (testResult.result?.snapshot?.pageId) {
                    const pageScreenshot = screenshotBlobs.get(testResult.result.snapshot.pageId);
                    if (pageScreenshot) return pageScreenshot;
                }

                // Check _allData for pageId
                if (testResult._allData && Array.isArray(testResult._allData)) {
                    for (const data of testResult._allData) {
                        if (data.pageId) {
                            const pageScreenshot = screenshotBlobs.get(data.pageId);
                            if (pageScreenshot) return pageScreenshot;
                        }
                    }
                }

                // Try to find matching screenshot by pageId substring
                const screenshotKeys = Array.from(screenshotBlobs.keys());
                if (testResult.other) {
                    const matchingKey = screenshotKeys.find(key => key.includes(testResult.other));
                    if (matchingKey) return screenshotBlobs.get(matchingKey);
                }

                // Fallback to last screenshot
                if (screenshotKeys.length > 0) {
                    return screenshotBlobs.get(screenshotKeys[screenshotKeys.length - 1]);
                }

                return undefined;
            };

            const screenshotUrl = findBestScreenshot(testResult);
            if (screenshotUrl) {
                testResult.passedScreenshot = screenshotUrl;
            }

            return testResult;
        });
    }

    // Filter and process errors
    const filteredErrors = shouldReturnPassedTest
        ? allErrors  // Don't filter passed tests, but screenshots are now assigned
        : filterRelevantErrors(allErrors.filter(error => error && error.callId));

    return {
        zipPath: file.name,
        errors: filteredErrors
    };
}


export async function analyzeTrace(file: File): Promise<CombinedReport> {
    const zipReport = await processZipFile(file);

    return {
        processedAt: new Date().toISOString(),
        comparison: processSingleZipForComparison(zipReport)
    };
}


function createSyntheticComparisonError(error: unknown): ComparisonDetail {
    const syntheticResult: TestResult = {
        testId: 'comparison-error',
        callId: 'error@comparison',
        type: 'error',
        error: {
            message: 'Failed to compare test traces: ' + (error instanceof Error ? error.message : 'Unknown error')
        },
        result: {
            log: [
                'Error encountered during comparison.',
                'Please check that all files are valid Playwright trace zip files.'
            ]
        }
    };

    return {
        callId: 'error@comparison',
        details: {
            failure: syntheticResult
        }
    };
}


function processSingleZipForComparison(report: ZipReport): ComparisonDetail[] {
    return report.errors.map(error => {
        const detail: ComparisonDetail = {
            callId: error.callId,
            details: {}
        };


        if (error.error) {
            detail.details.failure = error;
        } else {
            detail.details.passedTest = error;
        }

        return detail;
    });
}

// finished here
function compareZipReports(firstReport: ZipReport, secondReport: ZipReport): ComparisonDetail[] {
    const comparisonDetails: ComparisonDetail[] = [];

    // For each test in the first report
    for (const firstTest of firstReport.errors) {
        const secondTest = secondReport.errors.find(test => test.callId === firstTest.callId);


        if (secondTest) {
            const callId = firstTest.callId;

            if (firstTest.error && !secondTest.error) {
                comparisonDetails.push({
                    callId,
                    details: {
                        failure: firstTest,
                        passedTest: secondTest
                    }
                });
            } else if (!firstTest.error && secondTest.error) {
                comparisonDetails.push({
                    callId,
                    details: {
                        failure: secondTest,
                        passedTest: firstTest
                    }
                });
            }
        }
    }

    // remove _allData objects from comparison details
    comparisonDetails.forEach(detail => {
        if (detail.details.failure) {
            // @ts-ignore
            delete detail.details.failure._allData;
        }
        if (detail.details.passedTest) {
            // @ts-ignore
            delete detail.details.passedTest._allData;
        }
    });

    console.log(JSON.stringify(comparisonDetails));

    return comparisonDetails;
}


export async function compareTraces(files: File[]): Promise<CombinedReport> {
    const timestamp = new Date().toISOString();

    try {
        const zips = (await Promise.all(
            files.filter(f => f.name.endsWith(".zip"))
        )).sort((a, b) => b.size - a.size);

        const larger = zips[0];
        const smaller = zips.at(-1)!

        const failedTest = await processZipFile(larger);
        const passedTest = await processZipFile(smaller, true);

        return {
            processedAt: timestamp,
            comparison: compareZipReports(failedTest, passedTest)
        };
    } catch (error) {
        return {
            processedAt: timestamp,
            comparison: [createSyntheticComparisonError(error)]
        };
    }
}
