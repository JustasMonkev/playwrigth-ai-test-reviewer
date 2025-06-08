export function removeElements(htmlArray: any, elementsToRemove: string[]) {
    if (!Array.isArray(htmlArray)) {
        return;
    }

    for (let i = htmlArray.length - 1; i >= 0; i--) {
        if (Array.isArray(htmlArray[i])) {
            // If the element is an array, recursively process it.
            removeElements(htmlArray[i], elementsToRemove);
        } else if (typeof htmlArray[i] === 'string' && elementsToRemove.includes(htmlArray[i])) {
            // If it's a string and one of the elements to remove, remove it and the next two elements (attributes and content).
            htmlArray.splice(i, 3);
        }
    }
}

export function findMatchingResultsByCallId(callId: string, fileData: any) {
    try {
        const parsedData = JSON.parse(JSON.stringify(fileData));
        if (Array.isArray(parsedData)) {
            const rawData = parsedData;

            const relevantEntries = parsedData.filter((entry: any) => {
                const matchesCallId = entry.snapshot?.callId === callId || entry.callId === callId;
                const hasRelevantData = entry.apiName && entry.params && entry.params.selector;
                return matchesCallId || hasRelevantData;
            });

            // If no entries found, try to create a synthetic entry from test information
            if (relevantEntries.length === 0) {
                // Look for entries with test data
                const testEntries = parsedData.filter((entry: any) =>
                    entry.type === 'before' &&
                    entry.apiName &&
                    entry.params
                );

                if (testEntries.length > 0) {
                    // Create a synthetic test result
                    const testEntry = testEntries[0];

                    // Return a result that includes both the synthetic entry and the full raw data
                    const syntheticEntry = {
                        testId: testEntry.stepId || testEntry.callId,
                        callId: testEntry.callId || callId,
                        error: {
                            message: `Playwright assertion failed: ${testEntry.apiName} - Expected element matching "${testEntry.params?.selector || 'unknown selector'}" to be visible`
                        },
                        result: {
                            log: [
                                `Selector: ${testEntry.params?.selector || 'unknown'}`,
                                `Method: ${testEntry.method || 'unknown'}`,
                                `API: ${testEntry.apiName || 'unknown'}`,
                                `Expected: ${testEntry.params?.expression || 'unknown'}`
                            ]
                        },
                        // Store the complete raw entry for full access to data
                        rawEntry: testEntry,
                        // Store complete data for reference
                        allData: rawData
                    };

                    return [syntheticEntry];
                }

                // If still no entries, throw an error
                throw new Error('No matching results found');
            }

            const matchingResults = relevantEntries.map((entry: any) => {
                // Create a base result that captures ALL data but prepares it for display
                const baseResult = {
                    ...entry,  // Keep ALL original data
                    _rawEntry: entry,  // Store the original entry for debugging
                    _allData: rawData  // Store all data for reference
                };

                // Only transform frame-snapshot entries with HTML content
                if (entry.type === "frame-snapshot" && entry.snapshot && Array.isArray(entry.snapshot.html) && Number(entry.snapshot.viewport.width) > 0) {
                    const entryCopy = JSON.parse(JSON.stringify(entry)); // Deep copy
                    removeElements(entryCopy.snapshot.html, ["HEAD", "STYLE", "BODY", "META", "TITLE", "C-WIZ"]);

                    // Only return entries that still have HTML content after filtering
                    if (entryCopy.snapshot.html.length > 0) {
                        return {
                            ...baseResult,
                            ...entryCopy
                        };
                    }
                    // If no HTML content left, don't include this entry
                    return null;
                }

                // If it's a 'before' entry, enhance it with error information
                if (entry.type === "before" && entry.apiName) {
                    return {
                        ...baseResult,
                        testId: entry.stepId || entry.callId,
                        callId: entry.callId,
                        endTime: entry.startTime || 0,
                        error: {
                            message: `Playwright assertion failed: ${entry.apiName || 'Unknown assertion'} - Expected element matching "${entry.params?.selector || 'unknown selector'}" to be visible`
                        },
                        result: {
                            log: [
                                `Selector: ${entry.params?.selector || 'unknown'}`,
                                `Method: ${entry.method || 'unknown'}`,
                                `API: ${entry.apiName || 'unknown'}`,
                                `Expected: ${entry.params?.expression || 'unknown'}`
                            ]
                        },
                        // Keep access to the original entry
                        params: entry.params || {}
                    };
                }

                // Return other types unchanged but enhanced with raw data
                return baseResult;
            }).filter(Boolean); // Remove null entries

            if (!matchingResults.length) {
                throw new Error('No matching results after processing');
            }

            return matchingResults;
        } else {
            throw new Error('Input data is not an array');
        }
    } catch (error) {
        console.error('Error in findMatchingResultsByCallId:', error);

        // Create a synthetic result as a fallback
        return [{
            testId: 'synthetic-result',
            callId: callId || 'synthetic-call',
            error: {
                message: 'Failed to parse test results: ' + (error instanceof Error ? error.message : 'Unknown error')
            },
            result: {
                log: ['Error processing test results.']
            },
            // Include the raw data for debugging
            _rawData: fileData
        }];
    }
}

/**
 * Finds a specific entry in a JSON data string that contains a result with a log array and received data.
 * Now also collects and returns passed and failed tests as additional data.
 *
 * @param {any} jsonData - The JSON data string to search within.
 * @returns {any | null} Object containing the matching entry, passedTests, failedTests, and raw data,
 *   or null if error, or undefined if no relevant data found.
 *
 * @example
 * const jsonData = `[
 *   {"result": {"log": ["log1", "log2"], "received": "data1"}},
 *   {"result": {"log": ["log3"], "received": "data2"}}
 * ]`;
 * const result = findMatchingResult(jsonData);
 * // result will contain the matching entry with additional passedTests and failedTests arrays.
 */
/**
 * Finds a specific entry in a JSON data string that contains a result with a log array and received data.
 *
 * @param {any} jsonData - The JSON data string to search within.
 * @returns {any | null} The matching entry if found, otherwise null.
 *
 * @example
 * const jsonData = `[
 *   {"result": {"log": ["log1", "log2"], "received": "data1"}},
 *   {"result": {"log": ["log3"], "received": "data2"}}
 * ]`;
 * const result = findMatchingResult(jsonData);
 * // result will be the first entry.
 */
export function findMatchingResult(jsonData: any): any | null {
    const indexData = JSON.parse(JSON.stringify(jsonData));

    // First check for entries with a result object containing a log array and received data (failed tests)
    for (const entry of indexData) {
        if (entry.result &&
            entry.result.log instanceof Array &&
            entry.result.received) {

            // Return with additional raw data
            return {
                ...entry,
                _rawData: indexData
            };
        }
    }

    let snapshotID: string | undefined;
    for (const entry of indexData) {
        if (entry.result && entry.result.matches) {
            snapshotID = entry.afterSnapshot
        }

        if (entry.type === "frame-snapshot" && entry.snapshot.snapshotName === snapshotID) {
            // If it's a frame-snapshot, we can return it directly
            return {
                ...entry,
            };
        }
    }

    return undefined;
}
