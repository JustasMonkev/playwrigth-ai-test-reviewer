import React, { ButtonHTMLAttributes } from 'react';

interface BaseProps {
    children?: React.ReactNode;
    className?: string;
}

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, BaseProps {
    variant?: 'default' | 'primary' | 'outline' | 'destructive';
    size?: 'sm' | 'md' | 'lg';
}

export const Button = ({
    children,
    variant = "default",
    size = "md",
    className = "",
    ...props
}: ButtonProps) => {
    const variantClasses = {
        default: "bg-gradient-to-r from-slate-50 to-slate-100 hover:from-slate-100 hover:to-slate-200 text-slate-800 border border-slate-200",
        primary: "bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700 text-white shadow-blue-500/25",
        outline: "bg-white/80 backdrop-blur-sm border border-slate-200 hover:bg-slate-50 text-slate-800",
        destructive: "bg-gradient-to-r from-red-600 to-rose-600 hover:from-red-700 hover:to-rose-700 text-white shadow-red-500/25",
    };

    const sizeClasses = {
        sm: "text-xs px-3 py-1.5",
        md: "text-sm px-4 py-2",
        lg: "text-base px-6 py-3",
    };

    const baseClasses = `
        rounded-lg
        font-medium
        transition-all
        duration-300
        shadow-lg
        hover:shadow-xl
        focus:outline-none 
        focus:ring-2 
        focus:ring-offset-2 
        focus:ring-blue-500
        active:scale-[0.98]
        inline-flex 
        items-center 
        justify-center
        disabled:opacity-60
        disabled:pointer-events-none
    `;

    return (
        <button
            className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
            {...props}
        >
            {children}
        </button>
    );
};