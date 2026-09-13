type Props = {
  title?: string;
  description?: string;
};

export function EmptyState({
  title = '暂无数据',
  description,
}: Props) {
  return (
    <div className="empty">
      <div style={{ fontWeight: 600, color: 'var(--text)' }}>{title}</div>
      {description ? <div style={{ marginTop: 4 }}>{description}</div> : null}
    </div>
  );
}
