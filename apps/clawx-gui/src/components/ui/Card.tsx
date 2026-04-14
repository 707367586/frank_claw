import type { HTMLAttributes, ReactNode } from "react";
export function Card({ className = "", children, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`ui-card ${className}`.trim()} {...rest}>{children}</div>;
}
export function CardHeader({ children }: { children: ReactNode }) { return <div className="ui-card__header">{children}</div>; }
export function CardTitle({ children }: { children: ReactNode }) { return <h3 className="ui-card__title">{children}</h3>; }
export function CardContent({ children }: { children: ReactNode }) { return <div className="ui-card__content">{children}</div>; }
export function CardFooter({ children }: { children: ReactNode }) { return <div className="ui-card__footer">{children}</div>; }
