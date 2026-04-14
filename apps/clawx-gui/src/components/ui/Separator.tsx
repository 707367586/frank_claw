export default function Separator({ orientation = "horizontal" }: { orientation?: "horizontal" | "vertical" }) {
  return <div className={`ui-separator ui-separator--${orientation}`} role="separator" />;
}
