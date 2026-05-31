import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default:
          "bg-accent text-white hover:bg-accent/90",
        secondary:
          "bg-bg-elev-2 text-fg-primary hover:bg-bg-elev-2/80 border border-border",
        ghost:
          "hover:bg-bg-elev-2 text-fg-secondary hover:text-fg-primary",
        destructive:
          "bg-danger text-white hover:bg-danger/90",
        link:
          "text-accent underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 px-3 py-1",
        sm: "h-7 px-2 text-xs",
        lg: "h-10 px-6 text-md",
        icon: "h-8 w-8",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
