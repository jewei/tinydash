package platform

import (
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
)

// maxDesktopEntryBytes bounds the desktop entry read; a longer file is not
// a desktop entry, whose translations make it some tens of KB at most.
const maxDesktopEntryBytes = 1 << 20

// ownDesktopIDs are TinyDash's own entries, which the launcher does not
// list.
var ownDesktopIDs = []string{"TinyDash.desktop", "tinydash.desktop", "dev.tinydash.launcher.desktop"}

// launcherWrappers only start another program: Flatpak exports run
// `flatpak run ...` and many Snap entries `env VAR=... /snap/bin/x`, so
// their name would be the same alias for every such app.
var launcherWrappers = []string{"flatpak", "snap", "env", "sh", "bash"}

// AppFolders are the XDG application folders, including the Flatpak and
// Snap exports that XDG_DATA_DIRS lists.
func AppFolders() []string {
	home, _ := os.UserHomeDir()
	return xdgAppFolders(home, os.Getenv("XDG_DATA_HOME"), os.Getenv("XDG_DATA_DIRS"))
}

// xdgAppFolders puts the user's data folder first, as XDG ranks it above
// the system's. The XDG spec treats an empty variable as unset.
func xdgAppFolders(home, dataHome, dataDirs string) []string {
	if dataHome == "" {
		dataHome = filepath.Join(home, ".local", "share")
	}
	if dataDirs == "" {
		dataDirs = "/usr/local/share:/usr/share"
	}
	folders := []string{filepath.Join(dataHome, "applications")}
	for _, dir := range strings.Split(dataDirs, ":") {
		if dir != "" {
			folders = append(folders, filepath.Join(dir, "applications"))
		}
	}
	return folders
}

// InstalledApps lists the desktop entries that menus show, as GIO did for
// the Rust app.
func InstalledApps() (apps []App, ok bool) {
	env := desktopEnv{
		languages: languageNames(os.Getenv),
		desktops:  splitNonEmpty(os.Getenv("XDG_CURRENT_DESKTOP"), ":"),
		canRun:    func(program string) bool { _, err := exec.LookPath(program); return err == nil },
	}
	return discoverApps(AppFolders(), env), true
}

// desktopEnv is what decides how an entry shows in this session.
type desktopEnv struct {
	// languages are the locale names to look for, best first, as
	// languageNames gives them.
	languages []string
	// desktops are the names of XDG_CURRENT_DESKTOP.
	desktops []string
	// canRun reports whether a program is found and can run.
	canRun func(program string) bool
}

// discoverApps reads the desktop entries of folders, in their subfolders
// too. An entry's desktop ID is its path in the folder with / as -; the
// first folder that has an ID wins, also with an entry that is hidden or
// broken, as the XDG spec says.
func discoverApps(folders []string, env desktopEnv) []App {
	var apps []App
	seen := map[string]bool{}
	for _, folder := range folders {
		filepath.WalkDir(folder, func(path string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() || !strings.HasSuffix(d.Name(), ".desktop") {
				return nil
			}
			rel, err := filepath.Rel(folder, path)
			if err != nil {
				return nil
			}
			id := strings.ReplaceAll(filepath.ToSlash(rel), "/", "-")
			if seen[id] {
				return nil
			}
			seen[id] = true
			text, err := readLimited(path)
			if err != nil {
				return nil
			}
			if app, ok := desktopApp(id, path, text, env); ok {
				apps = append(apps, app)
			}
			return nil
		})
	}
	return apps
}

func readLimited(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, maxDesktopEntryBytes+1))
	if err != nil {
		return "", err
	}
	if len(data) > maxDesktopEntryBytes {
		return "", fs.ErrInvalid
	}
	return string(data), nil
}

// desktopApp is the app of a desktop entry, or false where GIO did not
// list it or menus do not show it: not an application, Hidden, NoDisplay,
// not for this desktop, a TryExec or Exec program that is not installed,
// or TinyDash itself. The name is in the user's language; Keywords and
// the program's name are aliases; GenericName (short, such as "Web
// Browser"), or else Comment, says what it is for.
func desktopApp(id, path, text string, env desktopEnv) (App, bool) {
	entry := parseDesktopEntry(text)
	if entry["Type"] != "Application" || isTrue(entry["Hidden"]) || isTrue(entry["NoDisplay"]) || slices.Contains(ownDesktopIDs, id) {
		return App{}, false
	}
	if !showsIn(entry, env.desktops) {
		return App{}, false
	}
	if try := unescape(entry["TryExec"]); try != "" && !env.canRun(try) {
		return App{}, false
	}
	var program string
	if line := unescape(entry["Exec"]); line != "" {
		first, ok := firstArgument(line)
		if !ok || !env.canRun(first) {
			return App{}, false
		}
		program = first
	}
	name, ok := localized(entry, "X-GNOME-FullName", env.languages)
	if !ok || name == "" {
		name, _ = localized(entry, "Name", env.languages)
	}
	if name == "" {
		return App{}, false
	}
	app := App{Name: name, Path: path}
	if keywords, ok := localizedRaw(entry, "Keywords", env.languages); ok {
		app.Aliases = splitList(keywords)
	}
	if base := filepath.Base(program); program != "" && !slices.Contains(launcherWrappers, base) {
		app.Aliases = append(app.Aliases, base)
	}
	description, ok := localized(entry, "GenericName", env.languages)
	if !ok {
		description, _ = localized(entry, "Comment", env.languages)
	}
	if strings.TrimSpace(description) != "" {
		app.Description = description
	}
	return app, true
}

// parseDesktopEntry gives the raw values of the [Desktop Entry] group, by
// key with its locale, such as Name[de]. A later key replaces an earlier
// one, as GLib reads it.
func parseDesktopEntry(text string) map[string]string {
	entry := map[string]string{}
	group := ""
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		switch {
		case line == "" || strings.HasPrefix(line, "#"):
		case strings.HasPrefix(line, "["):
			if group == "Desktop Entry" {
				return entry
			}
			group = strings.TrimSuffix(strings.TrimPrefix(line, "["), "]")
		case group == "Desktop Entry":
			if key, value, ok := strings.Cut(line, "="); ok {
				entry[strings.TrimSpace(key)] = strings.TrimSpace(value)
			}
		}
	}
	return entry
}

func isTrue(value string) bool { return value == "true" || value == "1" }

// showsIn applies OnlyShowIn and NotShowIn as GIO does: the first of the
// current desktops that either names decides; with none, an entry shows
// unless it has OnlyShowIn.
func showsIn(entry map[string]string, desktops []string) bool {
	only, hasOnly := entry["OnlyShowIn"]
	not := splitList(entry["NotShowIn"])
	for _, desktop := range desktops {
		if hasOnly && slices.Contains(splitList(only), desktop) {
			return true
		}
		if slices.Contains(not, desktop) {
			return false
		}
	}
	return !hasOnly
}

// localizedRaw is the raw value of key in the first of languages that the
// entry has, else the value without a locale.
func localizedRaw(entry map[string]string, key string, languages []string) (string, bool) {
	for _, language := range languages {
		if value, ok := entry[key+"["+language+"]"]; ok {
			return value, true
		}
	}
	value, ok := entry[key]
	return value, ok
}

func localized(entry map[string]string, key string, languages []string) (string, bool) {
	value, ok := localizedRaw(entry, key, languages)
	return unescape(value), ok
}

// unescape reads the escapes of a desktop entry string: \s, \n, \t, \r,
// and \\.
func unescape(value string) string {
	if !strings.Contains(value, `\`) {
		return value
	}
	var b strings.Builder
	for i := 0; i < len(value); i++ {
		if value[i] != '\\' || i+1 == len(value) {
			b.WriteByte(value[i])
			continue
		}
		i++
		switch value[i] {
		case 's':
			b.WriteByte(' ')
		case 'n':
			b.WriteByte('\n')
		case 't':
			b.WriteByte('\t')
		case 'r':
			b.WriteByte('\r')
		default:
			b.WriteByte(value[i])
		}
	}
	return b.String()
}

// splitList splits a list value at the semicolons that are not escaped
// (\;), and leaves out empty items.
func splitList(value string) []string {
	var items []string
	var item strings.Builder
	flush := func() {
		if s := unescape(item.String()); s != "" {
			items = append(items, s)
		}
		item.Reset()
	}
	for i := 0; i < len(value); i++ {
		switch {
		case value[i] == '\\' && i+1 < len(value) && value[i+1] == ';':
			item.WriteByte(';')
			i++
		case value[i] == '\\' && i+1 < len(value):
			item.WriteString(value[i : i+2])
			i++
		case value[i] == ';':
			flush()
		default:
			item.WriteByte(value[i])
		}
	}
	flush()
	return items
}

// firstArgument is the program of an Exec line: its first word, with
// shell quoting as GLib reads it. It is false for a line without one or
// with a quote that does not close.
func firstArgument(line string) (string, bool) {
	line = strings.TrimLeft(line, " \t\n")
	var word strings.Builder
	for i := 0; i < len(line); i++ {
		c := line[i]
		switch c {
		case ' ', '\t', '\n':
			return word.String(), word.Len() > 0
		case '\\':
			if i+1 < len(line) {
				i++
				if line[i] != '\n' {
					word.WriteByte(line[i])
				}
			} else {
				word.WriteByte(c)
			}
		case '\'':
			end := strings.IndexByte(line[i+1:], '\'')
			if end < 0 {
				return "", false
			}
			word.WriteString(line[i+1 : i+1+end])
			i += end + 1
		case '"':
			i++
			for ; i < len(line) && line[i] != '"'; i++ {
				if line[i] == '\\' && i+1 < len(line) && strings.IndexByte("$`\"\\\n", line[i+1]) >= 0 {
					i++
				}
				word.WriteByte(line[i])
			}
			if i == len(line) {
				return "", false
			}
		default:
			word.WriteByte(c)
		}
	}
	return word.String(), word.Len() > 0
}

// languageNames are the locale names for the user's messages, best
// first, as GLib gives them: from the first of LANGUAGE (a list), LC_ALL,
// LC_MESSAGES, and LANG that is set, each with its variants, such as
// de_DE@euro, de_DE, de@euro, and de for de_DE.UTF-8@euro.
func languageNames(getenv func(string) string) []string {
	var value string
	for _, name := range []string{"LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"} {
		if value = getenv(name); value != "" {
			break
		}
	}
	var names []string
	for _, locale := range splitNonEmpty(value, ":") {
		for _, variant := range localeVariants(locale) {
			if !slices.Contains(names, variant) {
				names = append(names, variant)
			}
		}
	}
	return names
}

// localeVariants of lang_TERRITORY.CODESET@MODIFIER, most specific first,
// in GLib's order: the territory counts most, then the codeset, then the
// modifier.
func localeVariants(locale string) []string {
	rest, modifier, hasModifier := strings.Cut(locale, "@")
	rest, codeset, hasCodeset := strings.Cut(rest, ".")
	language, territory, hasTerritory := strings.Cut(rest, "_")
	if language == "" || language == "C" || language == "POSIX" {
		return nil
	}
	var variants []string
	for mask := 7; mask >= 0; mask-- {
		if mask&4 != 0 && !hasTerritory || mask&2 != 0 && !hasCodeset || mask&1 != 0 && !hasModifier {
			continue
		}
		v := language
		if mask&4 != 0 {
			v += "_" + territory
		}
		if mask&2 != 0 {
			v += "." + codeset
		}
		if mask&1 != 0 {
			v += "@" + modifier
		}
		variants = append(variants, v)
	}
	return variants
}

func splitNonEmpty(value, separator string) []string {
	var items []string
	for _, item := range strings.Split(value, separator) {
		if item != "" {
			items = append(items, item)
		}
	}
	return items
}
