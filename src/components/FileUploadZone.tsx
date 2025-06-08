import React from 'react';
import { Upload } from 'lucide-react';
import { Card } from './ui/Card';
import { Badge } from './ui/Badge';

interface FileUploadZoneProps {
    uploadMode: string;
    setUploadMode: (mode: string) => void;
    handleFileDrop: (e: React.DragEvent<HTMLDivElement> | React.ChangeEvent<HTMLInputElement>) => void;
}

export const FileUploadZone: React.FC<FileUploadZoneProps> = ({
    uploadMode,
    setUploadMode,
    handleFileDrop
}) => {
    return (
        <Card className="overflow-hidden">
            <div
                className="border-2 border-dashed border-blue-200 bg-blue-150 rounded-lg p-12 text-center cursor-pointer hover:border-blue-400 hover:bg-blue-100 transition-colors duration-300"
                onDrop={handleFileDrop}
                onDragOver={(e) => e.preventDefault()}
                onClick={() => document.getElementById('file-input')?.click()}
            >
                <div className="mx-auto h-16 w-16 flex items-center justify-center bg-blue-100 rounded-full mb-4">
                    <Upload className="h-8 w-8 text-blue-600"/>
                </div>
                <h3 className="text-lg font-medium text-blue-900 mb-2">
                    {uploadMode === 'single' ? 'Upload ZIP File for Analysis' : 'Upload Two ZIP Files for Comparison'}
                </h3>
                <p className="text-blue-600 mb-4">Drag and drop files here, or click to browse</p>
                <div className="flex justify-center">
                    <Badge variant={uploadMode === 'single' ? 'success' : 'default'}>
                        Single File Analysis
                    </Badge>
                    <span className="mx-2 text-gray-400">or</span>
                    <Badge variant={uploadMode === 'multiple' ? 'success' : 'default'}>
                        Two-File Comparison
                    </Badge>
                </div>
                <input
                    id="file-input"
                    type="file"
                    className="hidden"
                    multiple={uploadMode === 'multiple'}
                    accept=".zip"
                    onChange={handleFileDrop}
                />
            </div>
            <div className="bg-white p-4 border-t border-gray-100 flex flex-col">
                <div className="flex justify-between items-center mb-4">
                    <p className="text-sm text-gray-500">
                        {uploadMode === 'single'
                            ? 'Single file mode: Upload one ZIP file for individual analysis'
                            : 'Comparison mode: Upload two ZIP files to compare test results'}
                    </p>
                    <div className="flex space-x-2">
                        <button
                            style={{cursor: 'pointer'}}
                            className={`text-sm py-1 px-3 rounded-full ${
                                uploadMode === 'single'
                                    ? 'bg-green-100 text-green-800 font-medium'
                                    : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
                            }`}
                            onClick={() => setUploadMode('single')}
                        >
                            Single File Analysis
                        </button>
                        <button
                            style={{cursor: 'pointer'}}
                            className={`text-sm py-1 px-3 rounded-full ${
                                uploadMode === 'multiple'
                                    ? 'bg-green-100 text-green-800 font-medium'
                                    : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
                            }`}
                            onClick={() => setUploadMode('multiple')}
                        >
                            Two-File Comparison
                        </button>
                    </div>
                </div>
                <div className="border-t border-gray-100 pt-4">
                    <h4 className="text-sm font-medium text-blue-700 mb-2">About Trace Viewer ZIP Files</h4>
                    <p className="text-xs text-gray-600 leading-relaxed">
                        Upload Playwright Trace Viewer ZIP files generated during test runs. These files contain
                        detailed test execution data including snapshots, network activity, and console logs.
                        To generate a trace file, use <code className="bg-gray-100 px-1 py-0.5 rounded text-blue-600">playwright test --trace on</code>
                        and locate the resulting <code className="bg-gray-100 px-1 py-0.5 rounded text-blue-600">.zip</code> file in your <code className="bg-gray-100 px-1 py-0.5 rounded text-blue-600">test-results/</code> directory.
                    </p>
                </div>
            </div>
        </Card>
    );
};
