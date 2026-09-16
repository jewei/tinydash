export default function AppAvatar(props: { name: string }) {
  const initials = () => {
    const words = props.name.trim().split(/\s+/);
    return words.length > 1
      ? `${Array.from(words[0])[0] ?? ""}${Array.from(words[1])[0] ?? ""}`.toUpperCase()
      : Array.from(props.name).slice(0, 2).join("").toUpperCase();
  };
  return (
    <span class="app-avatar" aria-hidden="true">
      {initials()}
    </span>
  );
}
