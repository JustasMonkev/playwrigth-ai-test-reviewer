import React from 'react';

interface BaseProps {
    children?: React.ReactNode;
    className?: string;
}

interface AlertProps extends BaseProps {
    variant?: 'default' | 'destructive' | 'warning' | 'success';
    icon?: React.ReactNode;
}

export const Alert = ({ children, variant = "default", className = "", icon }: AlertProps) => {
    const variantClasses = {
        default: "bg-gradient-to-r from-blue-50/90 to-indigo-50/90 border-blue-200 text-blue-800",
        destructive: "bg-gradient-to-r from-red-50/90 to-rose-50/90 border-red-200 text-red-800",
        warning: "bg-gradient-to-r from-amber-50/90 to-orange-50/90 border-amber-200 text-amber-800",
        success: "bg-gradient-to-r from-green-50/90 to-emerald-50/90 border-green-200 text-green-800",
    };

    const baseClasses = `
        backdrop-blur-sm
        shadow-lg
        transition-all
        duration-300
        hover:shadow-xl
        border
        rounded-xl
    `;

    return (
        <div className={`${baseClasses} p-5 flex items-start ${variantClasses[variant]} ${className}`}>
            {icon && (
                <div className="mr-4 mt-0.5 flex-shrink-0 opacity-90">
                    {icon}
                </div>
            )}
            <div className="flex-1">{children}</div>
        </div>
    );
};

export const AlertTitle = ({ children, className = "" }: BaseProps) => (
    <h4 className={`text-lg font-semibold mb-2 ${className}`}>{children}</h4>
);

export const AlertDescription = ({ children, className = "" }: BaseProps) => (
    <div className={`text-sm opacity-90 leading-relaxed ${className}`}>{children}</div>
);