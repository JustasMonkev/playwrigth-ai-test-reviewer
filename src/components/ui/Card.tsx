import React from 'react';

// Base props interface for reuse
interface BaseProps {
    children?: React.ReactNode;
    className?: string;
}

// Card component props
type CardProps = BaseProps;

export const Card = ({ children, className = "" }: CardProps) => (
    <div className={`bg-white rounded-lg shadow-md border border-gray-200 transition-all duration-200 hover:shadow-lg ${className}`}>
        {children}
    </div>
);

// CardHeader component props
type CardHeaderProps = BaseProps;

export const CardHeader = ({ children, className = "" }: CardHeaderProps) => (
    <div className={`px-6 py-4 border-b border-gray-100 bg-gray-50 rounded-t-lg ${className}`}>
        {children}
    </div>
);

// CardContent component props
type CardContentProps = BaseProps;

export const CardContent = ({ children, className = "" }: CardContentProps) => (
    <div className={`px-6 py-4 ${className}`}>{children}</div>
);