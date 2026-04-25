interface Props { open: boolean; onClose: () => void }
export default function CreateAgentModal({ open, onClose }: Props) {
  if (!open) return null;
  return (
    <div role="dialog" onClick={onClose}>
      <p>create agent</p>
    </div>
  );
}
