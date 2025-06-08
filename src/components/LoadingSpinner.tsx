import React from 'react';

export const LoadingSpinner: React.FC = () => {
    return (
        <div className="text-center py-16 bg-white rounded-lg shadow-md">
            <div className="animate-spin rounded-full h-16 w-16 border-b-2 border-t-2 border-blue-600 mx-auto"></div>
            <p className="mt-6 text-lg text-gray-700 font-medium">Processing files...</p>
            <p className="mt-2 text-gray-500">This may take a moment depending on file size</p>
        </div>
    );
};