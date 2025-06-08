import React from 'react';

interface BaseProps {
    children?: React.ReactNode;
    className?: string;
}

interface BadgeProps extends BaseProps {
    variant?: 'default' | 'success' | 'error' | 'warning';
}

export const Badge = ({ children, variant = "default" }: BadgeProps) => {
    const variantClasses = {
        default: "bg-gray-100 text-gray-800",
        success: "bg-green-100 text-green-800",
        error: "bg-red-100 text-red-800",
        warning: "bg-amber-100 text-amber-800",
    };

    return (
        <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${variantClasses[variant]}`}>
            {children}
        </span>
    );
};
