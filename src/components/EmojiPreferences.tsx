import { For } from "solid-js";

const tones = [
  { value: 0, name: "Default", example: "👍" },
  { value: 1, name: "Light", example: "👍🏻" },
  { value: 2, name: "Medium-light", example: "👍🏼" },
  { value: 3, name: "Medium", example: "👍🏽" },
  { value: 4, name: "Medium-dark", example: "👍🏾" },
  { value: 5, name: "Dark", example: "👍🏿" },
];
const languages = [
  { id: "zh", name: "Simplified Chinese · 简体中文" },
  { id: "ms", name: "Malay · Bahasa Melayu" },
  { id: "es", name: "Spanish · Español" },
];

export default function EmojiPreferences(props: {
  skinTone: number;
  languages: string[];
  onSkinTone: (value: number) => void;
  onLanguages: (value: string[]) => void;
}) {
  return (
    <div class="settings-group settings-separated">
      <h2>Emoji</h2>
      <label class="settings-row">
        <span>
          <strong>Preferred skin tone</strong>
          <span class="settings-hint">
            Use this tone for supported emoji. A typed emoji keeps its exact
            variant.
          </span>
        </span>
        <select
          aria-label="Preferred skin tone"
          value={props.skinTone}
          onChange={(event) =>
            props.onSkinTone(Number(event.currentTarget.value))
          }
        >
          <For each={tones}>
            {(tone) => (
              <option value={tone.value}>
                {tone.example} {tone.name}
              </option>
            )}
          </For>
        </select>
      </label>
      <p class="settings-hint" aria-label="Skin tone preview">
        Example: {tones.find((tone) => tone.value === props.skinTone)?.example}
      </p>
      <fieldset class="category-choices" aria-describedby="emoji-language-note">
        <legend>Emoji search languages</legend>
        <div class="category-choice-grid">
          <For each={languages}>
            {(language) => (
              <label
                class={{
                  "category-choice": true,
                  "is-selected": props.languages.includes(language.id),
                }}
              >
                <input
                  type="checkbox"
                  checked={props.languages.includes(language.id)}
                  onChange={(event) =>
                    props.onLanguages(
                      event.currentTarget.checked
                        ? [...props.languages, language.id]
                        : props.languages.filter((id) => id !== language.id),
                    )
                  }
                />
                <span>{language.name}</span>
              </label>
            )}
          </For>
        </div>
      </fieldset>
      <p id="emoji-language-note" class="settings-hint">
        English names and shortcodes are always available. Extra keywords work
        offline; result names stay in English.
      </p>
    </div>
  );
}
