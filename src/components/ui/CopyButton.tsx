import {useState} from 'react';
import {Copy} from 'lucide-react';
import {Button} from "./Button.tsx";

const CopyButton = ({handleCopyRawData}: any) => {
    const [showToast, setShowToast] = useState(false);

    const handleClick = () => {
        handleCopyRawData();
        setShowToast(true);

        setTimeout(() => {
            setShowToast(false);
        }, 400);
    };

    return (
        <>
            <Button
                className="absolute top-3 right-4"
                onClick={handleClick}
            >
                <Copy className="h-5 w-5"/>
            </Button>

            {showToast && (
                <div
                    className="fixed inset-0 flex items-center justify-center z-50"
                >
                    <div
                        className="max-w-xs bg-white border border-gray-200 rounded-xl shadow-lg dark:bg-neutral-800 dark:border-neutral-700 animate-in fade-in zoom-in duration-300"
                        role="alert"
                        aria-labelledby="toast-message"
                    >
                        <div className="flex p-4">
                            <div className="shrink-0">
                                <svg className="shrink-0 size-4 text-blue-500 mt-0.5" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
                                    <path d="M8 16A8 8 0 1 0 8 0a8 8 0 0 0 0 16zm.93-9.412-1 4.705c-.07.34.029.533.304.533.194 0 .487-.07.686-.246l-.088.416c-.287.346-.92.598-1.465.598-.703 0-1.002-.422-.808-1.319l.738-3.468c.064-.293.006-.399-.287-.47l-.451-.081.082-.381 2.29-.287zM8 5.5a1 1 0 1 1 0-2 1 1 0 0 1 0 2z"></path>
                                </svg>
                            </div>
                            <div className="ms-3">
                                <p id="toast-message" className="text-sm text-gray-700 dark:text-neutral-400">
                                    Raw data copied to clipboard!
                                </p>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </>
    );
};

export default CopyButton;
