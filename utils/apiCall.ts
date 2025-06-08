export const apiKey = import.meta.env.VITE_GEMINI_API_KEY

export const systemPrompt = `You are an expert Playwright test debugger. Analyze the following test failure and provide insights.\n\nThe test data includes:\n- Error message\n- Call logs showing the sequence of actions\n- Test parameters\n\nPlease provide your analysis in Markdown format with the following sections:\n\n- **Confidence Level**: High, Medium, or Low based on your confidence in the analysis\n\n- **Error Cause**: Clear explanation of what caused the test to fail\n\n- **Fix Suggestion**: Concrete suggestion for fixing the test, described in plain language. Do not provide actual code snippets.\n\nExamine timing, selectors, expectations, and errors carefully. Focus on actionable insights.\n\nImportant: Do not provide code to the user. Your suggestions should be descriptive explanations only.`;

export const userMessage = (errorMessage: string, testData: any, testParams: any): string => `
Here is the test failure data:

## Error Message:
${errorMessage}

## Call Logs: 
\`\`\`json
${JSON.stringify(testData)}
\`\`\`

${testParams ? `
## Test Parameters:
\`\`\`json
${JSON.stringify(testParams)}
\`\`\`
` : ''}

Please analyze this failure and provide recommendations in the requested MARKDOWN format.
`;
