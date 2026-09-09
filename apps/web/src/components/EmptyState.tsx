type Props = {
  title?: string;
  description?: string;
};

export function EmptyState({
  title = '暂无数据',
  description = '当后端可用并产生任务后，这里会显示内容。',
}: Props) {
  return (
    <div className="empty">
      <div style={{ fontSize: 28, marginBottom: 8, opacity: 0.35 }}>∅</div>
      <div style={{ fontWeight: 600, color: 'var(--text-secondary)' }}>{title}</div>
      <div style={{ marginTop: 4 }}>{description}</div>
    </div>
  );
}
